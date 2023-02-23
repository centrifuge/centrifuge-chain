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

use cfg_traits::PoolInspect;
use cfg_utils::vec_to_fixed_array;
use codec::{Decode, Encode, EncodeLike, Input, MaxEncodedLen};
use frame_support::traits::{
	fungibles::{Inspect, Mutate, Transfer},
	OriginTrait,
};
use orml_traits::asset_registry::{self, Inspect as _};
pub use pallet::*;
use scale_info::TypeInfo;
use sp_core::{TypeId, U256};
use sp_runtime::{traits::AtLeast32BitUnsigned, FixedPointNumber};
use sp_std::{boxed::Box, convert::TryInto, vec, vec::Vec};
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
	/// Moonbeam - It may be Moonbeam on Polkadot, Moonriver on Kusama, or Moonbase on a testnet.
	Moonbeam,
}

/// A Domain is a chain or network we can send a Connectors message to.
/// The domain indices need to match those used in the EVM contracts and these
/// need to pass the Centrifuge domain to send tranche tokens from the other
/// domain here. Therefore, DO NOT remove or move variants around.
#[derive(Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Domain {
	/// Referring to the Centrifuge Parachain. Will be used for handling incoming messages.
	/// NOTE: Connectors messages CAN NOT be sent directly from the Centrifuge chain to the
	/// Centrifuge chain itself.
	Centrifuge,
	/// An EVM domain, identified by its EVM Chain Id
	EVM(EVMChainId),
}

impl Encode for Domain {
	fn encode(&self) -> Vec<u8> {
		match self {
			Self::Centrifuge => vec![0; 9],
			Self::EVM(chain_id) => {
				let mut output: Vec<u8> = 1u8.encode();
				output.append(&mut chain_id.to_be_bytes().to_vec());

				output
			}
		}
	}
}

impl EncodeLike for Domain {}

impl Decode for Domain {
	fn decode<I: Input>(input: &mut I) -> Result<Self, codec::Error> {
		let variant = input.read_byte()?;

		match variant {
			0 => Ok(Self::Centrifuge),
			1 => {
				let mut chain_id_be_bytes = [0; 8];
				input.read(&mut chain_id_be_bytes[..])?;

				let chain_id = EVMChainId::from_be_bytes(chain_id_be_bytes);
				Ok(Self::EVM(chain_id))
			}
			_ => Err(codec::Error::from("Unknown Domain variant")),
		}
	}
}

/// The EVM Chain ID
/// The type should accomodate all chain ids listed on https://chainlist.org/.
type EVMChainId = u64;

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
pub struct DomainLocator<Domain> {
	pub domain: Domain,
}

impl<Domain> TypeId for DomainLocator<Domain> {
	const TYPE_ID: [u8; 4] = *b"domn";
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum DomainAddress {
	/// A Centrifuge-Chain based account address, 32-bytes long
	Centrifuge([u8; 32]),
	/// An EVM chain address, 20-bytes long
	EVM(EVMChainId, [u8; 20]),
}

impl From<DomainAddress> for Domain {
	fn from(x: DomainAddress) -> Self {
		match x {
			DomainAddress::Centrifuge(_) => Domain::Centrifuge,
			DomainAddress::EVM(chain_id, _) => Domain::EVM(chain_id),
		}
	}
}

impl DomainAddress {
	/// Get the address in a 32-byte long representation.
	/// For EVM addresses, append 12 zeros.
	fn address(&self) -> [u8; 32] {
		match self.clone() {
			Self::Centrifuge(x) => x,
			Self::EVM(_, x) => vec_to_fixed_array(x.to_vec()),
		}
	}

	fn domain(&self) -> Domain {
		self.clone().into()
	}
}

impl TypeId for DomainAddress {
	const TYPE_ID: [u8; 4] = *b"dadr";
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

pub type CurrencyIdOf<T> = <T as pallet_xcm_transactor::Config>::CurrencyId;

#[frame_support::pallet]
pub mod pallet {
	use cfg_primitives::Moment;
	use cfg_traits::{Permissions, PoolInspect, TrancheCurrency};
	use cfg_types::{
		permissions::{PermissionScope, PoolRole, Role},
		tokens::CustomMetadata,
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

		/// The origin allowed to make admin-like changes, such calling `set_domain_router`.
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


	/// The set of known connectors. This set is used as an allow-list when authorizing
	/// the origin of incoming messages through the `handle` extrinsic.
	#[pallet::storage]
	pub(crate) type KnownConnectors<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, ()>;

	#[pallet::error]
	pub enum Error<T> {
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set a Domain's router
		#[pallet::weight(< T as Config >::WeightInfo::set_domain_router())]
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

		/// Add an AccountId to the set of known connectors, allowing that origin
		/// to send incoming messages.
		#[pallet::weight(< T as Config >::WeightInfo::add_pool())]
		pub fn add_connector(origin: OriginFor<T>, connector: T::AccountId) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			<KnownConnectors<T>>::insert(connector, ());

			Ok(())
		}

		/// Add a pool to a given domain
		#[pallet::weight(< T as Config >::WeightInfo::add_pool())]
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
		pub fn add_tranche(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
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
				Message::UpdateTokenPrice {
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
					Role::PoolRole(PoolRole::MemberListAdmin)
				),
				BadOrigin
			);

			// Now add the destination address as a TrancheInvestor of the given tranche if
			// not already one. This check is necessary shall a user have called `update_member`
			// already but the call has failed on the EVM side and needs to be retried.
			if !T::Permission::has(
				PermissionScope::Pool(pool_id),
				domain_address.into_account_truncating(),
				Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, valid_until)),
			) {
				T::Permission::add(
					PermissionScope::Pool(pool_id),
					domain_address.into_account_truncating(),
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, valid_until)),
				)?;
			}

			Self::do_send_message(
				who,
				Message::UpdateMember {
					pool_id,
					tranche_id,
					valid_until,
					address: domain_address.address(),
				},
				domain_address.domain(),
			)?;

			Ok(())
		}

		/// Transfer tranche tokens to a given address
		#[pallet::weight(< T as Config >::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			domain_address: DomainAddress,
			amount: <T as pallet::Config>::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// Check that the destination is a member of this tranche token
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					domain_address.into_account_truncating(),
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
				who,
				Message::Transfer {
					pool_id,
					tranche_id,
					amount,
					domain: domain_address.domain(),
					address: domain_address.address(),
				},
				domain_address.domain(),
			)?;

			Ok(())
		}

		/// Handle an incoming message
		/// TODO(nuno): we probably need a custom origin type for these messages to ensure they have
		/// come in through XCM. Probably even handle it in a separate pallet? For now, let's have a
		/// POC here to test the pipeline Ethereum ---> Moonbeam ---> Centrifuge::connectors
		#[pallet::call_index(99)]
		#[pallet::weight(< T as Config >::WeightInfo::add_pool())]
		pub fn handle(origin: OriginFor<T>, message: Vec<u8>) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			ensure!(<KnownConnectors<T>>::contains_key(&sender), Error::<T>::InvalidIncomingMessageOrigin);

			Self::deposit_event(Event::IncomingMessage { sender, message });
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

			let contract_call = contract::encoded_contract_call(message.encode());
			let ethereum_xcm_call =
				Self::encoded_ethereum_xcm_call(xcm_domain.clone(), contract_call);

			pallet_xcm_transactor::Pallet::<T>::transact_through_sovereign(
				T::RuntimeOrigin::root(),
				// The destination to which the message should be sent
				Box::new(xcm_domain.location),
				fee_payer,
				// The currency in which we want to pay fees
				CurrencyPayment {
					currency: Currency::AsCurrencyId(xcm_domain.fee_currency),
					fee_amount: None,
				},
				// The call to be executed in the destination chain
				ethereum_xcm_call,
				OriginKind::SovereignAccount,
				TransactWeights {
					// Convert the max gas_limit into a max transact weight following Moonbeam's formula.
					transact_required_weight_at_most: xcm_domain.max_gas_limit * 25_000
						+ 100_000_000,
					overall_weight: None,
				},
			)?;

			Self::deposit_event(Event::MessageSent { message, domain });

			Ok(())
		}

		/// Build the encoded `ethereum_xcm::transact(eth_tx)` call that should
		/// request to execute `evm_call`.
		///
		/// * `xcm_domain` - All the necessary info regarding the xcm-based domain
		/// where this `ethereum_xcm` call is to be executed
		/// * `evm_call` - The encoded EVM call calling ConnectorsXcmRouter::handle(msg)
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
	}
}

#[cfg(test)]
mod tests {
	use cfg_primitives::AccountId;
	use codec::{Decode, Encode};
	use sp_runtime::traits::AccountIdConversion;

	use super::DomainAddress;
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

	#[test]
	fn domain_address_account_derivation() {
		assert_eq!(
			account_from(DomainAddress::EVM(1284, [9; 20])),
			account_from(DomainAddress::EVM(1284, [9; 20])),
		);

		assert_ne!(
			account_from(DomainAddress::EVM(1284, [42; 20])),
			account_from(DomainAddress::EVM(1284, [24; 20])),
		);

		assert_ne!(
			account_from(DomainAddress::EVM(1284, [9; 20])),
			account_from(DomainAddress::EVM(1285, [9; 20])),
		);
	}

	fn account_from(domain_address: DomainAddress) -> AccountId {
		domain_address.into_account_truncating()
	}
}
