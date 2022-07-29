//! Centrifuge Connectors pallet
//!
//! TODO(nuno): add description
//!
//!
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode, HasCompact};
use common_traits::PoolInspect;
use frame_support::dispatch::DispatchResult;
use frame_support::traits::fungibles::{Inspect, Mutate, Transfer};
use frame_system::ensure_signed;
use scale_info::TypeInfo;
use sp_core::TypeId;
use sp_runtime::traits::AtLeast32BitUnsigned;
use sp_runtime::FixedPointNumber;
use sp_std::convert::TryInto;

pub use pallet::*;

pub mod weights;

mod message;
pub use message::*;

mod routers;
pub use routers::*;

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Domain {
	Centrifuge,
	Moonbeam,
	Ethereum,
	Avalanche,
	Gnosis,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
pub struct DomainLocator<Domain> {
	pub domain: Domain,
}

impl<Domain> TypeId for DomainLocator<Domain> {
	const TYPE_ID: [u8; 4] = *b"domn";
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct DomainAddress<Domain> {
	pub domain: Domain,
	pub address: [u8; 32],
}

impl<Domain> TypeId for DomainAddress<Domain> {
	const TYPE_ID: [u8; 4] = *b"dadr";
}

// Type aliases
pub type PoolIdOf<T> = <<T as Config>::PoolInspect as PoolInspect<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
>>::PoolId;

pub type TrancheIdOf<T> = <<T as Config>::PoolInspect as PoolInspect<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
>>::TrancheId;

pub type MessageOf<T> =
	Message<Domain, PoolIdOf<T>, TrancheIdOf<T>, <T as Config>::Balance, <T as Config>::Rate>;

pub type CurrencyIdOf<T> = <T as pallet_xcm_transactor::Config>::CurrencyId;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;
	use common_traits::{Moment, Permissions, PoolInspect};
	use common_types::{CurrencyId, PermissionScope, PoolRole, Role};
	use frame_support::{error::BadOrigin, pallet_prelude::*, traits::UnixTime};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::AccountIdConversion;
	use sp_runtime::traits::Zero;
	use xcm::v0::MultiLocation;
	use xcm::v2::OriginKind;
	use xcm::VersionedMultiLocation;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_xcm_transactor::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type WeightInfo: WeightInfo;

		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		type Rate: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber + TypeInfo;

		type CurrencyId: Parameter
			+ Copy
			// TODO(nuno): remove Default is not needed after MVP tests
			+ Default
			+ IsType<<Self as pallet_xcm_transactor::Config>::CurrencyId>;

		type AdminOrigin: EnsureOrigin<Self::Origin>;

		type PoolInspect: PoolInspect<
			Self::AccountId,
			<Self as pallet::Config>::CurrencyId,
			Rate = Self::Rate,
		>;

		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<PoolIdOf<Self>, <Self as pallet::Config>::CurrencyId>,
			Role = Role<TrancheIdOf<Self>, Moment>,
			Error = DispatchError,
		>;

		type Time: UnixTime;

		type Tokens: Mutate<Self::AccountId>
			+ Inspect<
				Self::AccountId,
				AssetId = <Self as pallet::Config>::CurrencyId,
				Balance = <Self as pallet::Config>::Balance,
			> + Transfer<Self::AccountId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A message was sent to a domain
		MessageSent {
			domain: Domain,
			message: MessageOf<T>,
		},
	}

	#[pallet::storage]
	pub(crate) type DomainRouter<T: Config> = StorageMap<_, Blake2_128Concat, Domain, Router>;

	#[pallet::error]
	pub enum Error<T> {
		/// A pool could not be found
		PoolNotFound,
		/// A tranche could not be found
		TrancheNotFound,
		/// The specified domain has no associated router
		InvalidDomain,
		/// Failed to send a message
		SendFailure,
		/// Token price is not set
		MissingPrice,
		/// The router does not exist
		MissingRouter,
		/// The selected domain is not yet supported
		UnsupportedDomain,
		/// Transfer amount must be non-zero
		InvalidTransferAmount,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a pool to a given domain
		#[pallet::weight(<T as Config>::WeightInfo::add_pool())]
		pub fn add_pool(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			domain: Domain,
		) -> DispatchResult {
			ensure_signed(origin.clone())?;

			// Check the pool exists
			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);

			// Send the message through the router
			Self::do_send_message(Message::AddPool { pool_id }, domain)?;

			Ok(())
		}

		/// Add a tranche to a given domain
		#[pallet::weight(<T as Config>::WeightInfo::add_tranche())]
		pub fn add_tranche(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			domain: Domain,
		) -> DispatchResult {
			ensure_signed(origin.clone())?;

			// Check the tranche exists
			ensure!(
				T::PoolInspect::tranche_exists(pool_id, tranche_id),
				Error::<T>::TrancheNotFound
			);

			// Send the message through the router
			//
			// TODO: retrieve token name and symbol from asset-registry
			// Depends on https://github.com/centrifuge/centrifuge-chain/issues/842
			//
			// TODO: only allow calling add_tranche when
			// both the name and symbol are non-zero values.
			Self::do_send_message(
				Message::AddTranche {
					pool_id,
					tranche_id,
					token_name: [0; 32],
					token_symbol: [0; 32],
				},
				domain,
			)?;

			Ok(())
		}

		/// Update a token price
		#[pallet::weight(<T as Config>::WeightInfo::update_token_price())]
		pub fn update_token_price(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			domain: Domain,
		) -> DispatchResult {
			ensure_signed(origin.clone())?;

			// Check the tranche exists
			ensure!(
				T::PoolInspect::tranche_exists(pool_id, tranche_id),
				Error::<T>::TrancheNotFound
			);

			// Get the current price
			let latest_price = T::PoolInspect::get_tranche_token_price(pool_id, tranche_id)
				.ok_or(Error::<T>::MissingPrice)?;

			// Send the message through the router
			Self::do_send_message(
				Message::UpdateTokenPrice {
					pool_id,
					tranche_id,
					price: latest_price.price,
				},
				domain,
			)?;

			Ok(())
		}

		/// Update a member
		#[pallet::weight(<T as Config>::WeightInfo::update_member())]
		pub fn update_member(
			origin: OriginFor<T>,
			address: DomainAddress<Domain>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			valid_until: Moment,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// Check that the origin is a member of this tranche token or is a memberlist admin and thus allowed to add other members.
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, Self::now()))
				) || T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::MemberListAdmin)
				),
				BadOrigin
			);

			T::Permission::add(
				PermissionScope::Pool(pool_id),
				address.into_account_truncating(),
				Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, valid_until)),
			)?;

			// Send the message through the router
			Self::do_send_message(
				Message::UpdateMember {
					pool_id,
					tranche_id,
					valid_until,
					address: address.address,
				},
				address.domain,
			)?;

			Ok(())
		}

		/// Update a member
		#[pallet::weight(<T as Config>::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			address: DomainAddress<Domain>,
			amount: <T as pallet::Config>::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// Check that the destination is a member of this tranche token
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					address.into_account_truncating(),
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, Self::now()))
				),
				BadOrigin
			);

			// Check whether amount > 0
			ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);

			// TODO: Transfer to the domain account for bookkeeping
			// T::Tokens::transfer(
			// 	T::CurrencyId::Tranche(pool_id, tranche_id),
			// 	&who,
			// 	&DomainLocator {
			// 		domain: address.domain,
			// 	}
			// 	.into_account_truncating(),
			// 	amount,
			// 	false,
			// )?;

			// Send the message through the router
			Self::do_send_message(
				Message::Transfer {
					pool_id,
					tranche_id,
					amount,
					domain: address.clone().domain,
					destination: address.clone().address,
				},
				address.domain,
			)?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn now() -> Moment {
			T::Time::now().as_secs()
		}

		pub fn do_set_router(domain: Domain, router: Router) -> DispatchResult {
			<DomainRouter<T>>::insert(domain.clone(), router);
			Ok(())
		}

		pub fn do_send_message(message: MessageOf<T>, domain: Domain) -> Result<(), Error<T>> {
			let router = <DomainRouter<T>>::get(domain.clone()).ok_or(Error::<T>::MissingRouter)?;

			match router {
				Router::Xcm { location } => Self::send_through_xcm(&message, location),
				_ => Err(Error::<T>::UnsupportedDomain.into()),
			}?;

			Self::deposit_event(Event::MessageSent { message, domain });
			Ok(())
		}

		fn send_through_xcm(
			message: &MessageOf<T>,
			dest_location: VersionedMultiLocation,
		) -> Result<(), Error<T>> {
			use codec::Encode;
			use frame_support::traits::OriginTrait;
			use Default;

			todo!()
		}

		//TODO(nuno): this is just for testing purposes for now.
		pub fn send_through_xcm_tests(
			message: &MessageOf<T>,
			dest_location: VersionedMultiLocation,
			fees_location: VersionedMultiLocation,
			fee_payer: T::AccountId,
		) -> DispatchResult {
			use codec::Encode;
			use frame_support::traits::OriginTrait;
			use sp_std::boxed::Box;

			pallet_xcm_transactor::Pallet::<T>::transact_through_sovereign(
				T::Origin::root(),
				Box::new(dest_location),
				fee_payer,
				Box::new(fees_location),
				8_000_000_000_000,
				(*message).encode().to_vec(),
				OriginKind::SovereignAccount,
			)
		}
	}
}
