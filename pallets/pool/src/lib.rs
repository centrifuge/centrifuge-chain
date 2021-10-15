//! # Pool pallet for runtime
//!
//! This pallet provides functionality for managing a tinlake pool
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::sp_runtime::traits::{AccountIdConversion, AtLeast32Bit, One};
use std::fmt::Debug;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use frame_support::traits::{EnsureOrigin, Get};
use frame_system::pallet_prelude::OriginFor;
use orml_traits::MultiCurrency;
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// The data structure for storing Pool data
#[derive(Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct PoolData<AccountID> {
	pub creator: AccountID,
	pub name: String,
}

pub type CurrencyIdOf<T> = <<T as pallet::Config>::MultiCurrency as MultiCurrency<
	<T as frame_system::Config>::AccountId,
>>::CurrencyId;

pub type MultiCurrencyBalanceOf<T> = <<T as pallet::Config>::MultiCurrency as MultiCurrency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_nft::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The overarching poolID type
		type PoolId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Debug
			+ Default
			+ Copy
			+ AtLeast32Bit;

		type LoanId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Copy
			+ IsType<<Self as pallet_nft::Config>::TokenId>;

		type MultiCurrency: orml_traits::MultiCurrency<<Self as frame_system::Config>::AccountId>;

		/// Origin that can make transfers possible
		type TransferOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;

		/// PalletID of this pool module
		#[pallet::constant]
		type PoolPalletId: Get<PalletId>;
	}

	/// Stores the PoolInfo against a poolID
	#[pallet::storage]
	#[pallet::getter(fn get_pool_info)]
	pub(super) type PoolInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, PoolData<T::AccountId>, OptionQuery>;

	#[pallet::type_value]
	pub fn OnNextPoolIDEmpty<T: Config>() -> T::PoolId {
		// always start the token ID from 1 instead of zero
		T::PoolId::one()
	}
	/// Stores the next pool_id that will be created.
	#[pallet::storage]
	#[pallet::getter(fn get_pool_nonce)]
	pub(super) type PoolNonce<T: Config> =
		StorageValue<_, T::PoolId, ValueQuery, OnNextPoolIDEmpty<T>>;

	/// Stores the pool_id to currencyId
	#[pallet::storage]
	#[pallet::getter(fn get_pool_currency)]
	pub(super) type PoolCurrency<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, CurrencyIdOf<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// PoolCreated is emitted when a new pool is created
		PoolCreated(T::PoolId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when the pool associated with a pool_id is missing
		ErrMissingPool,

		/// Emits when the pool currency is missing
		ErrMissingPoolCurrency,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the given fee for the key
		#[pallet::weight(100_000)]
		pub fn create_pool(
			origin: OriginFor<T>,
			name: String,
			currency_id: CurrencyIdOf<T>,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let pool_id = Self::create_new_pool(creator, name, currency_id);
			Self::deposit_event(Event::PoolCreated(pool_id));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	// checks if the pool associated with pool_id exists
	pub fn check_pool(pool_id: T::PoolId) -> DispatchResult {
		PoolInfo::<T>::get(pool_id).ok_or(Error::<T>::ErrMissingPool)?;
		Ok(())
	}

	pub fn create_new_pool(
		creator: T::AccountId,
		name: String,
		currency_id: CurrencyIdOf<T>,
	) -> T::PoolId {
		let pd = PoolData { creator, name };
		let pool_id = PoolNonce::<T>::get();
		PoolInfo::<T>::insert(pool_id, pd);
		let next_pool_id = pool_id + T::PoolId::one();
		PoolNonce::<T>::set(next_pool_id);
		PoolCurrency::<T>::insert(pool_id, currency_id);
		pool_id
	}

	/// returns the account_id of the pool pallet
	pub fn account_id() -> T::AccountId {
		T::PoolPalletId::get().into_account()
	}

	pub fn borrow_currency(
		pool_id: T::PoolId,
		origin: OriginFor<T>,
		to: T::AccountId,
		amount: MultiCurrencyBalanceOf<T>,
	) -> DispatchResult {
		// must be a loan origin
		T::TransferOrigin::ensure_origin(origin)?;
		let currency_id =
			PoolCurrency::<T>::get(pool_id).ok_or(Error::<T>::ErrMissingPoolCurrency)?;
		T::MultiCurrency::transfer(currency_id, &Self::account_id(), &to, amount)
	}

	pub fn repay_currency(
		pool_id: T::PoolId,
		origin: OriginFor<T>,
		from: T::AccountId,
		amount: MultiCurrencyBalanceOf<T>,
	) -> DispatchResult {
		T::TransferOrigin::ensure_origin(origin)?;
		let currency_id =
			PoolCurrency::<T>::get(pool_id).ok_or(Error::<T>::ErrMissingPoolCurrency)?;
		T::MultiCurrency::transfer(currency_id, &from, &Self::account_id(), amount)
	}
}
