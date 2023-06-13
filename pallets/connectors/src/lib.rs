// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
#![cfg_attr(not(feature = "std"), no_std)]
use core::convert::TryFrom;

use cfg_traits::{connectors::Codec, PoolInspect};
use cfg_types::domain_address::{Domain, DomainAddress, DomainLocator};
use cfg_utils::vec_to_fixed_array;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::{
	fungibles::{Inspect, Mutate, Transfer},
	OriginTrait,
};
use orml_traits::asset_registry::{self, Inspect as _};
pub use pallet::*;
use scale_info::TypeInfo;
use sp_core::U256;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Convert},
	FixedPointNumber,
};
use sp_std::{convert::TryInto, vec, vec::Vec};
pub mod weights;

mod message;
pub use message::*;

mod routers;
pub use routers::*;

mod contract;
pub use contract::*;

/// The Parachains that Centrifuge Connectors support.
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum ParachainId {
	/// Moonbeam - It may be Moonbeam on Polkadot, Moonriver on Kusama, or
	/// Moonbase on a testnet.
	Moonbeam,
}

// Type aliases
pub type PoolIdOf<T> = <<T as Config>::PoolInspect as PoolInspect<
	<T as frame_system::Config>::AccountId,
	CurrencyIdOf<T>,
>>::PoolId;

pub type TrancheIdOf<T> = <<T as Config>::PoolInspect as PoolInspect<
	<T as frame_system::Config>::AccountId,
	CurrencyIdOf<T>,
>>::TrancheId;

pub type MessageOf<T> =
	Message<Domain, PoolIdOf<T>, TrancheIdOf<T>, <T as Config>::Balance, <T as Config>::Rate>;

pub type CurrencyIdOf<T> = <T as Config>::CurrencyId;

#[frame_support::pallet]
pub mod pallet {
	use cfg_primitives::Moment;
	use cfg_traits::{Permissions, PoolInspect, TrancheCurrency};
	use cfg_types::{
		permissions::{PermissionScope, PoolRole, Role},
		tokens::{CustomMetadata, GeneralCurrencyIndex},
	};
	use frame_support::{error::BadOrigin, pallet_prelude::*, traits::UnixTime};
	use frame_system::pallet_prelude::*;
	use pallet_xcm_transactor::{Currency, CurrencyPayment, TransactWeights};
	use sp_runtime::traits::{AccountIdConversion, Zero};
	use xcm::latest::OriginKind;

	use super::*;
	use crate::weights::WeightInfo;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_xcm_transactor::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type WeightInfo: WeightInfo;

		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		type Rate: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber + TypeInfo;

		/// The origin allowed to make admin-like changes, such calling
		/// `set_domain_router`.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type PoolInspect: PoolInspect<Self::AccountId, CurrencyIdOf<Self>, Rate = Self::Rate>;

		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<PoolIdOf<Self>, CurrencyIdOf<Self>>,
			Role = Role<TrancheIdOf<Self>, Moment>,
			Error = DispatchError,
		>;

		type Time: UnixTime;

		type Tokens: Mutate<Self::AccountId>
			+ Inspect<
				Self::AccountId,
				AssetId = CurrencyIdOf<Self>,
				Balance = <Self as pallet::Config>::Balance,
			> + Transfer<Self::AccountId>;

		type TrancheCurrency: TrancheCurrency<PoolIdOf<Self>, TrancheIdOf<Self>>
			+ Into<CurrencyIdOf<Self>>;

		type AssetRegistry: asset_registry::Inspect<
			AssetId = CurrencyIdOf<Self>,
			Balance = <Self as Config>::Balance,
			CustomMetadata = CustomMetadata,
		>;

		/// The currency type of transferrable token.
		type CurrencyId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen
			+ Into<<Self as pallet_xcm_transactor::Config>::CurrencyId>
			+ TryInto<
				GeneralCurrencyIndex<u128, <Self as Config>::GeneralCurrencyPrefix>,
				Error = DispatchError,
			>;

		/// The converter from a DomainAddress to a Substrate AccountId
		type AccountConverter: Convert<DomainAddress, Self::AccountId>;

		/// The prefix for currencies added via Connectors.
		#[pallet::constant]
		type GeneralCurrencyPrefix: Get<[u8; 12]>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A message was sent to a domain
		MessageSent {
			message: MessageOf<T>,
			domain: Domain,
		},

		/// The Router for a given domain was set
		SetDomainRouter {
			domain: Domain,
			router: Router<CurrencyIdOf<T>>,
		},

		IncomingMessage {
			sender: T::AccountId,
			message: Vec<u8>,
		},
	}

	#[pallet::storage]
	pub(crate) type DomainRouter<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, Router<CurrencyIdOf<T>>>;

	/// The set of known connectors. This set is used as an allow-list when
	/// authorizing the origin of incoming messages through the `handle`
	/// extrinsic.
	#[pallet::storage]
	pub(crate) type KnownConnectors<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ()>;

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to map the asset to the corresponding Connector's General
		/// Index representation
		AssetNotFound,
		/// A pool could not be found
		PoolNotFound,
		/// A tranche could not be found
		TrancheNotFound,
		/// Could not find the metadata of a tranche token
		TrancheMetadataNotFound,
		/// Failed to fetch a tranche token price.
		/// This can occur if `TrancheNotFound` or if effectively
		/// the price for this tranche has not yet been set.
		MissingTranchePrice,
		/// Router not set for a given domain
		MissingRouter,
		/// Transfer amount must be non-zero
		InvalidTransferAmount,
		/// A transfer to a non-whitelisted destination was attempted
		UnauthorizedTransfer,
		/// Failed to build Ethereum_Xcm call
		FailedToBuildEthereumXcmCall,
		/// The origin of an incoming message is not in the allow-list
		InvalidIncomingMessageOrigin,
		/// Failed to decode an incoming message
		InvalidIncomingMessage,
		/// A transfer attempt from the local to the local domain
		InvalidTransferDomain,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set a Domain's router
		#[pallet::weight(< T as Config >::WeightInfo::set_domain_router())]
		#[pallet::call_index(0)]
		pub fn set_domain_router(
			origin: OriginFor<T>,
			domain: Domain,
			router: Router<CurrencyIdOf<T>>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			<DomainRouter<T>>::insert(domain.clone(), router.clone());
			Self::deposit_event(Event::SetDomainRouter { domain, router });

			Ok(())
		}

		/// Add an AccountId to the set of known connectors, allowing that
		/// origin to send incoming messages.
		#[pallet::weight(< T as Config >::WeightInfo::add_connector())]
		#[pallet::call_index(1)]
		pub fn add_connector(origin: OriginFor<T>, connector: T::AccountId) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			<KnownConnectors<T>>::insert(connector, ());

			Ok(())
		}

		/// Add a pool to a given domain
		#[pallet::weight(< T as Config >::WeightInfo::add_pool())]
		#[pallet::call_index(2)]
		pub fn add_pool(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			domain: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);

			Self::do_send_message(who, Message::AddPool { pool_id }, domain)?;

			Ok(())
		}

		/// Add a tranche to a given domain
		#[pallet::weight(< T as Config >::WeightInfo::add_tranche())]
		#[pallet::call_index(3)]
		pub fn add_tranche(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			decimals: u8,
			domain: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				T::PoolInspect::tranche_exists(pool_id, tranche_id),
				Error::<T>::TrancheNotFound
			);

			// Look up the metadata of the tranche token
			let currency_id = T::TrancheCurrency::generate(pool_id, tranche_id).into();
			let metadata = T::AssetRegistry::metadata(&currency_id)
				.ok_or(Error::<T>::TrancheMetadataNotFound)?;
			let token_name = vec_to_fixed_array(metadata.name);
			let token_symbol = vec_to_fixed_array(metadata.symbol);
			let price = T::PoolInspect::get_tranche_token_price(pool_id, tranche_id)
				.ok_or(Error::<T>::MissingTranchePrice)?
				.price;

			// Send the message to the domain
			Self::do_send_message(
				who,
				Message::AddTranche {
					pool_id,
					tranche_id,
					decimals,
					token_name,
					token_symbol,
					price,
				},
				domain,
			)?;

			Ok(())
		}

		/// Update a token price
		#[pallet::weight(< T as Config >::WeightInfo::update_token_price())]
		#[pallet::call_index(4)]
		pub fn update_token_price(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			domain: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let price = T::PoolInspect::get_tranche_token_price(pool_id, tranche_id)
				.ok_or(Error::<T>::MissingTranchePrice)?
				.price;

			Self::do_send_message(
				who,
				Message::UpdateTrancheTokenPrice {
					pool_id,
					tranche_id,
					price,
				},
				domain,
			)?;

			Ok(())
		}

		/// Update a member
		#[pallet::weight(< T as Config >::WeightInfo::update_member())]
		#[pallet::call_index(5)]
		pub fn update_member(
			origin: OriginFor<T>,
			domain_address: DomainAddress,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			valid_until: Moment,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// Check that the origin is allowed to add other members
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::InvestorAdmin)
				),
				BadOrigin
			);

			// Now add the destination address as a TrancheInvestor of the given tranche if
			// not already one. This check is necessary shall a user have called
			// `update_member` already but the call has failed on the EVM side and needs to
			// be retried.
			if !T::Permission::has(
				PermissionScope::Pool(pool_id),
				T::AccountConverter::convert(domain_address.clone()),
				Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, valid_until)),
			) {
				T::Permission::add(
					PermissionScope::Pool(pool_id),
					T::AccountConverter::convert(domain_address.clone()),
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, valid_until)),
				)?;
			}

			Self::do_send_message(
				who,
				Message::UpdateMember {
					pool_id,
					tranche_id,
					valid_until,
					member: domain_address.address(),
				},
				domain_address.domain(),
			)?;

			Ok(())
		}

		/// Transfer tranche tokens to a given address
		#[pallet::weight(< T as Config >::WeightInfo::transfer())]
		#[pallet::call_index(6)]
		pub fn transfer_tranche_tokens(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			domain_address: DomainAddress,
			amount: <T as pallet::Config>::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// Check that the destination is not the local domain
			ensure!(
				domain_address.domain() != Domain::Centrifuge,
				Error::<T>::InvalidTransferDomain
			);

			// Check that the destination is a member of this tranche token
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					T::AccountConverter::convert(domain_address.clone()),
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, Self::now()))
				),
				Error::<T>::UnauthorizedTransfer
			);

			ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);

			// Transfer to the domain account for bookkeeping
			T::Tokens::transfer(
				T::TrancheCurrency::generate(pool_id, tranche_id).into(),
				&who,
				&DomainLocator::<Domain> {
					domain: domain_address.domain(),
				}
				.into_account_truncating(),
				amount,
				false,
			)?;

			Self::do_send_message(
				who.clone(),
				Message::TransferTrancheTokens {
					pool_id,
					tranche_id,
					amount,
					domain: domain_address.domain(),
					sender: who
						.encode()
						.try_into()
						.map_err(|_| DispatchError::Other("Conversion to 32 bytes failed"))?,
					receiver: domain_address.address(),
				},
				domain_address.domain(),
			)?;

			Ok(())
		}

		/// Handle an incoming message
		/// TODO(nuno): we probably need a custom origin type for these messages
		/// to ensure they have come in through XCM. For now, let's have a POC
		/// here to test the pipeline Ethereum ---> Moonbeam --->
		/// Centrifuge::connectors
		#[pallet::call_index(99)]
		#[pallet::weight(< T as Config >::WeightInfo::handle())]
		pub fn handle(origin: OriginFor<T>, bytes: Vec<u8>) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			ensure!(
				<KnownConnectors<T>>::contains_key(&sender),
				Error::<T>::InvalidIncomingMessageOrigin
			);

			Self::deposit_event(Event::IncomingMessage {
				sender,
				message: bytes.clone(),
			});
			// todo: do someting with the decoded message later on
			let _: MessageOf<T> = Message::deserialize(&mut bytes.as_slice())
				.map_err(|_| Error::<T>::InvalidIncomingMessage)?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn now() -> Moment {
			T::Time::now().as_secs()
		}

		/// Send the `message` to the given domain.
		pub fn do_send_message(
			fee_payer: T::AccountId,
			message: MessageOf<T>,
			domain: Domain,
		) -> DispatchResult {
			let Router::Xcm(xcm_domain) =
				<DomainRouter<T>>::get(domain.clone()).ok_or(Error::<T>::MissingRouter)?;

			let contract_call = contract::encoded_contract_call(message.serialize());
			let ethereum_xcm_call =
				Self::encoded_ethereum_xcm_call(xcm_domain.clone(), contract_call);

			pallet_xcm_transactor::Pallet::<T>::transact_through_sovereign(
				T::RuntimeOrigin::root(),
				// The destination to which the message should be sent
				xcm_domain.location,
				fee_payer,
				// The currency in which we want to pay fees
				CurrencyPayment {
					currency: Currency::AsCurrencyId(xcm_domain.fee_currency.into()),
					fee_amount: None,
				},
				// The call to be executed in the destination chain
				ethereum_xcm_call,
				OriginKind::SovereignAccount,
				TransactWeights {
					// Convert the max gas_limit into a max transact weight following Moonbeam's
					// formula.
					transact_required_weight_at_most: Weight::from_ref_time(
						xcm_domain.max_gas_limit * 25_000 + 100_000_000,
					),
					overall_weight: None,
				},
			)?;

			Self::deposit_event(Event::MessageSent { message, domain });

			Ok(())
		}

		/// Build the encoded `ethereum_xcm::transact(eth_tx)` call that should
		/// request to execute `evm_call`.
		///
		/// * `xcm_domain` - All the necessary info regarding the xcm-based
		///   domain
		/// where this `ethereum_xcm` call is to be executed
		/// * `evm_call` - The encoded EVM call calling
		///   ConnectorsXcmRouter::handle(msg)
		pub fn encoded_ethereum_xcm_call(
			xcm_domain: XcmDomain<CurrencyIdOf<T>>,
			evm_call: Vec<u8>,
		) -> Vec<u8> {
			let mut encoded: Vec<u8> = Vec::new();

			encoded.append(
				&mut xcm_domain
					.ethereum_xcm_transact_call_index
					.clone()
					.into_inner(),
			);
			encoded.append(
				&mut xcm_primitives::EthereumXcmTransaction::V1(
					xcm_primitives::EthereumXcmTransactionV1 {
						gas_limit: U256::from(xcm_domain.max_gas_limit),
						fee_payment: xcm_primitives::EthereumXcmFee::Auto,
						action: pallet_ethereum::TransactionAction::Call(
							xcm_domain.contract_address,
						),
						value: U256::zero(),
						input: BoundedVec::<
							u8,
							ConstU32<{ xcm_primitives::MAX_ETHEREUM_XCM_INPUT_SIZE }>,
						>::try_from(evm_call)
						.unwrap(),
						access_list: None,
					},
				)
				.encode(),
			);

			encoded
		}

		/// Returns the `u128` general index of a currency as the concatenation
		/// of the configured `GeneralCurrencyPrefix` and its local currency
		/// identifier.
		///
		/// Assumes the currency to be registered in the `AssetRegistry`.
		pub fn try_get_general_index(currency: CurrencyIdOf<T>) -> Result<u128, DispatchError> {
			ensure!(
				T::AssetRegistry::metadata(&currency).is_some(),
				Error::<T>::AssetNotFound
			);

			let general_index: GeneralCurrencyIndex<u128, T::GeneralCurrencyPrefix> =
				CurrencyIdOf::<T>::try_into(currency)?;

			Ok(general_index.index)
		}
	}
}

#[cfg(test)]
mod tests {
	use codec::{Decode, Encode};

	use crate::Domain;

	#[test]
	fn test_domain_encode_decode() {
		test_domain_identity(Domain::Centrifuge);
		test_domain_identity(Domain::EVM(1284));
		test_domain_identity(Domain::EVM(1));
	}

	/// Test that decode . encode results in the original value
	fn test_domain_identity(domain: Domain) {
		let encoded = domain.encode();
		let decoded: Domain = Domain::decode(&mut encoded.as_slice()).expect("");

		assert_eq!(domain, decoded);
	}
}
