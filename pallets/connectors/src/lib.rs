//! Centrifuge Connectors pallet
//!
//! TODO(nuno): add rich description
//!
//!
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode, HasCompact};
use common_traits::PoolInspect;
use frame_support::dispatch::DispatchResult;
use frame_system::ensure_signed;
use scale_info::TypeInfo;
use sp_runtime::traits::AtLeast32BitUnsigned;
use sp_std::convert::TryInto;

pub use pallet::*;

pub mod weights;

mod message;
pub use message::*;

mod routers;
pub use routers::*;

// Type aliases
pub type PoolIdOf<T> =
	<<T as Config>::PoolInspect as PoolInspect<<T as frame_system::Config>::AccountId>>::PoolId;
pub type TrancheIdOf<T> =
	<<T as Config>::PoolInspect as PoolInspect<<T as frame_system::Config>::AccountId>>::TrancheId;
pub type MessageOf<T> = Message<PoolIdOf<T>, TrancheIdOf<T>>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;
	use common_traits::PoolInspect;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_core::TypeId;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type WeightInfo: WeightInfo;

		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + TypeInfo;

		type AdminOrigin: EnsureOrigin<Self::Origin>;

		//TODO(nuno)
		type Permissions: Member;

		//TODO(nuno)
		type PoolInspect: PoolInspect<Self::AccountId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A pool was added to the domain
		MessageSent {
			domain: Domain,
			message: Message<PoolIdOf<T>, TrancheIdOf<T>>,
		},
	}

	#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Debug))]
	pub enum Domain {
		Centrifuge,
		Moonbeam,
		Ethereum,
		Avalanche,
		Gnosis,
	}

	#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct DomainLocator<Domain> {
		pub domain: Domain,
	}

	impl<Domain> TypeId for DomainLocator<Domain> {
		const TYPE_ID: [u8; 4] = *b"domn";
	}

	#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Debug))]
	pub struct DomainAddress(pub [u8; 32]);

	#[pallet::storage]
	pub(crate) type Routers<T: Config> = StorageMap<_, Blake2_128Concat, Domain, Router>;

	#[pallet::storage]
	pub(crate) type LinkedAddressesByAccount<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		Domain,
		DomainAddress,
	>;

	#[pallet::storage]
	pub(crate) type LinkedAddresses<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, Domain, Blake2_128Concat, DomainAddress, bool>;

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
			// TODO: retrieve token name and symbol from asset-registry
			// Depends on https://github.com/centrifuge/centrifuge-chain/issues/842
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
	}

	impl<T: Config> Pallet<T> {
		// skeleton

		pub fn do_send_message(message: MessageOf<T>, domain: Domain) -> Result<(), Error<T>> {
			let router = <Routers<T>>::get(domain.clone()).ok_or(Error::<T>::InvalidDomain)?;
			router
				.send(domain, message)
				.map_err(|_| Error::<T>::SendFailure)
		}
	}
}
