// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchResult;
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::{traits::ConstU32, BoundedVec};
use sp_std::boxed::Box;
use xcm::VersionedMultiLocation;
use cfg_types::tokens::CurrencyId;

#[allow(clippy::derive_partial_eq_without_eq)] // XcmDomain does not impl Eq
#[derive(Debug, Encode, Decode, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Router<CurrencyId> {
	// An XCM-based router
	Xcm(XcmDomain<CurrencyId>),
}

trait RouterTrait {
	fn forward(sender, destination, msg) -> DispatchResult;
}

/// XcmDomain gathers all the required fields to build and send remote
/// calls to a specific XCM-based Domain.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct XcmDomain<CurrencyId> {
	/// The xcm multilocation of the domain
	pub location: Box<VersionedMultiLocation>,
	/// The ethereum_xcm::Call::transact call index on a given domain.
	/// It should contain the pallet index + the `transact` call index, to which
	/// we will append the eth_tx param. You can obtain this value by building
	/// an ethereum_xcm::transact call with Polkadot JS on the target chain.
	pub ethereum_xcm_transact_call_index:
		BoundedVec<u8, ConstU32<{ xcm_primitives::MAX_ETHEREUM_XCM_INPUT_SIZE }>>,
	/// The ConnectorsXcmRouter contract address on a given domain
	pub contract_address: H160,
	/// The currency in which execution fees will be paid on
	pub fee_currency: CurrencyId,
	/// The max gas_limit we want to propose for a remote evm execution
	pub max_gas_limit: u64,
}

// NOTE: Remove this custom implementation once the following underlying data
// implements MaxEncodedLen:
/// * Polkadot Repo: xcm::VersionedMultiLocation
/// * PureStake Repo: pallet_xcm_transactor::Config<Self = T>::CurrencyId
impl<CurrencyId> MaxEncodedLen for XcmDomain<CurrencyId>
where
	XcmDomain<CurrencyId>: Encode,
{
	fn max_encoded_len() -> usize {
		// The domain's `VersionedMultiLocation` (custom bound)
		xcm::v1::MultiLocation::max_encoded_len()
			// From the enum wrapping of `VersionedMultiLocation`
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
	}
}
