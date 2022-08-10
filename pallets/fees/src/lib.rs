//! # Fees pallet for runtime
//!
//! This pallet provides a storing functionality for setting and getting fees associated with an Hash key.
//! Fees can only be set by FeeOrigin or RootOrigin
//!
//! Also, for its internal usage from the runtime or other pallets,
//! it offers some utilities to transfer the fees to the author, the treasury or burn it.
#![cfg_attr(not(feature = "std"), no_std)]
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	traits::{Currency, EnsureOrigin, ExistenceRequirement, WithdrawReasons},
};
use frame_system::ensure_root;

pub use pallet::*;
#[cfg(test)]
mod mock;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod tests;

pub mod weights;
use frame_support::pallet_prelude::{Decode, Encode};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
pub use weights::*;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Different stored fees keys
#[derive(Encode, Decode, Clone, Copy, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum FeeKey {
	CommitAnchor,
	// Others keys for pallet_fees go here
}

/// A way to identify a fee value.
pub enum Fee<T: Config> {
	/// The fee value itself.
	Balance(BalanceOf<T>),
	/// The fee value is already stored in the pallet fee by a key.
	Key(FeeKey),
}

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_authorship::Config {
		/// The currency mechanism.
		type Currency: frame_support::traits::Currency<Self::AccountId>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Required origin for changing fees
		type FeeChangeOrigin: EnsureOrigin<Self::Origin>;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
	}

	// The genesis config type.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub initial_fees: Vec<(FeeKey, BalanceOf<T>)>,
	}

	// The default value for the genesis config type.
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				initial_fees: Default::default(),
			}
		}
	}

	// The build of genesis for the pallet.
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			for (key, fee) in self.initial_fees.iter() {
				<Fees<T>>::insert(key, fee);
			}
		}
	}

	/// Stores the Fees associated with a Hash identifier
	#[pallet::storage]
	#[pallet::getter(fn fee)]
	pub(super) type Fees<T: Config> = StorageMap<_, Blake2_256, FeeKey, BalanceOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		FeeChanged(FeeKey, BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Fee associated to given key not found
		FeeNotFoundForKey,
		/// Invalid author of blocked fetched
		InvalidAuthor,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the given fee for the key
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_fee())]
		pub fn set_fee(origin: OriginFor<T>, key: FeeKey, fee: BalanceOf<T>) -> DispatchResult {
			T::FeeChangeOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;

			<Fees<T>>::insert(key, fee);
			Self::deposit_event(Event::FeeChanged(key, fee));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Pay the fees to the block author.
	/// If the `from` account has not enough balance or the author is invalid the fees are not
	/// paid.
	pub fn fee_to_author(from: &T::AccountId, fee: Fee<T>) -> DispatchResult {
		let author = <pallet_authorship::Pallet<T>>::author().ok_or(Error::<T>::InvalidAuthor)?;
		let balance = Self::withdraw_fee(from, fee)?;
		T::Currency::resolve_creating(&author, balance);
		Ok(())
	}

	/// Burn the fees.
	/// If the `from` account has not enough balance the fees are not paid.
	pub fn fee_to_burn(from: &T::AccountId, fee: Fee<T>) -> DispatchResult {
		Self::withdraw_fee(from, fee).map(|_| ())
	}

	/// Transfer the fees to the treasury.
	/// If the `from` account has not enough balance the fees are not paid.
	pub fn fee_to_treasury(from: &T::AccountId, fee: Fee<T>) -> DispatchResult {
		todo!()
	}

	fn withdraw_fee(
		from: &T::AccountId,
		fee: Fee<T>,
	) -> Result<<T::Currency as Currency<T::AccountId>>::NegativeImbalance, DispatchError> {
		let balance = match fee {
			Fee::Balance(balance) => balance,
			Fee::Key(key) => <Fees<T>>::get(key).ok_or(Error::<T>::FeeNotFoundForKey)?,
		};

		T::Currency::withdraw(
			&from,
			balance,
			WithdrawReasons::FEE.into(),
			ExistenceRequirement::KeepAlive,
		)
	}
}
