// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use core::convert::TryFrom;

use cfg_traits::connectors::Codec;
use codec::{Decode, Encode, MaxEncodedLen};
use ethabi::{Bytes, Contract};
use frame_support::{
	dispatch::DispatchResult, sp_runtime::DispatchError, traits::OriginTrait, weights::Weight,
};
use pallet_xcm_transactor::{Currency, CurrencyPayment, TransactWeights};
use scale_info::TypeInfo;
use sp_core::{bounded::BoundedVec, ConstU32, H160, U256};
use sp_std::{boxed::Box, marker::PhantomData, vec, vec::Vec};
use xcm::{
	v2::{MultiLocation, OriginKind},
	VersionedMultiLocation,
};

use crate::{AccountIdOf, CurrencyIdOf, MessageOf};

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EthereumXCMRouter<T>
where
	T: frame_system::Config + pallet_xcm_transactor::Config + pallet_connectors_gateway::Config,
{
	pub xcm_domain: XcmDomain<CurrencyIdOf<T>>,
	pub _marker: PhantomData<T>,
}

impl<T> EthereumXCMRouter<T>
where
	T: frame_system::Config + pallet_xcm_transactor::Config + pallet_connectors_gateway::Config,
{
	pub fn do_init(&self) -> DispatchResult {
		pallet_xcm_transactor::Pallet::<T>::set_transact_info(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			self.xcm_domain.location.clone(),
			self.xcm_domain.transact_info.transact_extra_weight,
			self.xcm_domain.transact_info.max_weight,
			self.xcm_domain.transact_info.transact_extra_weight_signed,
		)?;

		pallet_xcm_transactor::Pallet::<T>::set_fee_per_second(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			self.xcm_domain.fee_asset_location.clone(),
			self.xcm_domain.fee_per_second,
		)
	}

	pub fn do_send(&self, sender: AccountIdOf<T>, msg: MessageOf<T>) -> DispatchResult {
		let contract_call = get_encoded_contract_call(msg.serialize())
			.map_err(|_| DispatchError::Other("encoded contract call retrieval"))?;

		let ethereum_xcm_call =
			get_encoded_ethereum_xcm_call::<T>(self.xcm_domain.clone(), contract_call)
				.map_err(|_| DispatchError::Other("encoded ethereum xcm call retrieval"))?;

		pallet_xcm_transactor::Pallet::<T>::transact_through_sovereign(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			// The destination to which the message should be sent.
			self.xcm_domain.location.clone(),
			// The sender will pay for this transaction.
			sender,
			// The currency in which we want to pay fees.
			CurrencyPayment {
				currency: Currency::AsCurrencyId(self.xcm_domain.fee_currency.clone()),
				fee_amount: None,
			},
			// The call to be executed in the destination chain.
			ethereum_xcm_call,
			OriginKind::SovereignAccount,
			TransactWeights {
				// Convert the max gas_limit into a max transact weight following
				// Moonbeam's formula.
				transact_required_weight_at_most: Weight::from_all(
					self.xcm_domain.max_gas_limit * 25_000 + 100_000_000,
				),
				overall_weight: None,
			},
		)?;

		Ok(())
	}
}

/// Build the encoded `ethereum_xcm::transact(eth_tx)` call that should
/// request to execute `evm_call`.
///
/// * `xcm_domain` - All the necessary info regarding the xcm-based domain
/// where this `ethereum_xcm` call is to be executed
/// * `evm_call` - The encoded EVM call calling ConnectorsXcmRouter::handle(msg)
pub(crate) fn get_encoded_ethereum_xcm_call<T>(
	xcm_domain: XcmDomain<CurrencyIdOf<T>>,
	evm_call: Vec<u8>,
) -> Result<Vec<u8>, ()>
where
	T: frame_system::Config + pallet_xcm_transactor::Config + pallet_connectors_gateway::Config,
{
	let input =
		BoundedVec::<u8, ConstU32<{ xcm_primitives::MAX_ETHEREUM_XCM_INPUT_SIZE }>>::try_from(
			evm_call,
		)
		.map_err(|_| ())?;

	let mut encoded: Vec<u8> = Vec::new();

	encoded.append(
		&mut xcm_domain
			.ethereum_xcm_transact_call_index
			.clone()
			.into_inner(),
	);
	encoded.append(
		&mut xcm_primitives::EthereumXcmTransaction::V1(xcm_primitives::EthereumXcmTransactionV1 {
			gas_limit: U256::from(xcm_domain.max_gas_limit),
			fee_payment: xcm_primitives::EthereumXcmFee::Auto,
			action: pallet_ethereum::TransactionAction::Call(xcm_domain.contract_address),
			value: U256::zero(),
			input,
			access_list: None,
		})
		.encode(),
	);

	Ok(encoded)
}

/// Return the encoded contract call, i.e,
/// ConnectorsXcmRouter::handle(encoded_msg).
pub(crate) fn get_encoded_contract_call(encoded_msg: Vec<u8>) -> Result<Bytes, ()> {
	let contract = get_xcm_router_contract();
	let encoded_contract_call = contract
		.function(HANDLE_FUNCTION)
		.map_err(|_| ())?
		.encode_input(&[ethabi::Token::Bytes(encoded_msg)])
		.map_err(|_| ())?;

	Ok(encoded_contract_call)
}

/// The ConnectorsXcmContract handle function name.
const HANDLE_FUNCTION: &str = "handle";

/// The ConnectorsXcmContract message param name.
const MESSAGE_PARAM: &str = "message";

/// The ConnectorsXcmRouter Abi as in ethabi::Contract
/// Note: We only concern ourselves with the `handle` function of the
/// contract since that's all we need to build the calls for remote EVM
/// execution.
pub(crate) fn get_xcm_router_contract() -> Contract {
	use sp_std::collections::btree_map::BTreeMap;

	let mut functions = BTreeMap::new();
	#[allow(deprecated)]
	functions.insert(
		HANDLE_FUNCTION.into(),
		vec![ethabi::Function {
			name: HANDLE_FUNCTION.into(),
			inputs: vec![ethabi::Param {
				name: MESSAGE_PARAM.into(),
				kind: ethabi::ParamType::Bytes,
				internal_type: None,
			}],
			outputs: vec![],
			constant: false,
			state_mutability: Default::default(),
		}],
	);

	Contract {
		constructor: None,
		functions,
		events: Default::default(),
		errors: Default::default(),
		receive: false,
		fallback: false,
	}
}

/// XcmDomain gathers all the required fields to build and send remote
/// calls to a specific XCM-based Domain.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
pub struct XcmDomain<CurrencyId> {
	/// The XCM multilocation of the domain
	pub location: Box<VersionedMultiLocation>,
	/// The ethereum_xcm::Call::transact call index on a given domain.
	/// It should contain the pallet index + the `transact` call index, to which
	/// we will append the eth_tx param. You can obtain this value by building
	/// an ethereum_xcm::transact call with Polkadot JS on the target chain.
	pub ethereum_xcm_transact_call_index:
		BoundedVec<u8, ConstU32<{ xcm_primitives::MAX_ETHEREUM_XCM_INPUT_SIZE }>>,
	/// The ConnectorsXcmRouter contract address on a given domain
	pub contract_address: H160,
	/// The max gas_limit we want to propose for a remote evm execution
	pub max_gas_limit: u64,
	/// The XCM transact info that will be stored in the
	/// `TransactInfoWithWeightLimit` storage of the XCM transactor pallet.
	pub transact_info: XcmTransactInfo,
	/// The currency in which execution fees will be paid on
	pub fee_currency: CurrencyId,
	/// The fee per second that will be stored in the
	/// `DestinationAssetFeePerSecond` storage of the XCM transactor pallet.
	pub fee_per_second: u128,
	/// The location of the asset used for paying XCM fees.
	pub fee_asset_location: Box<VersionedMultiLocation>,
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
/// XcmTransactInfo hold all the weight related information required for the XCM
/// transactor pallet.
pub struct XcmTransactInfo {
	pub transact_extra_weight: Weight,
	pub max_weight: Weight,
	pub transact_extra_weight_signed: Option<Weight>,
}

/// NOTE: Remove this custom implementation once the following underlying data
/// implements MaxEncodedLen:
/// * Polkadot Repo: xcm::VersionedMultiLocation
/// * PureStake Repo: pallet_xcm_transactor::Config<Self = T>::CurrencyId
impl<CurrencyId> MaxEncodedLen for XcmDomain<CurrencyId>
where
	XcmDomain<CurrencyId>: Encode,
{
	fn max_encoded_len() -> usize {
		// The domain's `VersionedMultiLocation` (custom bound)
		MultiLocation::max_encoded_len()
			// From the enum wrapping of `VersionedMultiLocation` for the XCM domain location.
			.saturating_add(1)
			// From the enum wrapping of `VersionedMultiLocation` for the asset fee location.
			.saturating_add(1)
			// The ethereum xcm call index (default bound)
			.saturating_add(BoundedVec::<
				u8,
				ConstU32<{ xcm_primitives::MAX_ETHEREUM_XCM_INPUT_SIZE }>,
			>::max_encoded_len())
			// The contract address (default bound)
			.saturating_add(H160::max_encoded_len())
			// The fee currency (custom bound)
			.saturating_add(cfg_types::tokens::CurrencyId::max_encoded_len())
			// The XcmTransactInfo
			.saturating_add(XcmTransactInfo::max_encoded_len())
	}
}
