//! # Fees pallet for runtime
//!
//! This pallet provides a storing functionality for setting and getting fees associated with an Hash key.
//! Fees can only be set by FeeOrigin or RootOrigin
//!
//! Also, for its internal usage from the runtime or other pallets,
//! it offers some utilities to transfer the fees to the author, the treasury or burn it.
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{EncodeLike, FullCodec};
use common_traits::fees::{self, Fee};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	storage::types::ValueQuery,
	traits::{Currency, EnsureOrigin, ExistenceRequirement, OnUnbalanced, WithdrawReasons},
};
use frame_system::ensure_root;
use scale_info::TypeInfo;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type ImbalanceOf<T> = <<T as Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

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
		/// Key type used for storing and identifying fees.
		type FeeKey: FullCodec
			+ TypeInfo
			+ MaybeSerializeDeserialize
			+ sp_std::fmt::Debug
			+ Clone
			+ Copy
			+ PartialEq;

		/// The currency mechanism.
		type Currency: Currency<Self::AccountId>;

		/// The treasury destination.
		type Treasury: OnUnbalanced<ImbalanceOf<Self>>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Required origin for changing fees.
		type FeeChangeOrigin: EnsureOrigin<Self::Origin>;

		/// Default value for fee keys.
		type DefaultFeeValue: Get<BalanceOf<Self>>;

		/// Type representing the weight of this pallet.
		type WeightInfo: WeightInfo;
	}

	// The genesis config type.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub initial_fees: Vec<(T::FeeKey, BalanceOf<T>)>,
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
				<FeeBalances<T>>::insert(key, fee);
			}
		}
	}

	/// Stores the fee balances associated with a Hash identifier
	#[pallet::storage]
	#[pallet::getter(fn fee)]
	pub(super) type FeeBalances<T: Config> =
		StorageMap<_, Blake2_256, T::FeeKey, BalanceOf<T>, ValueQuery, T::DefaultFeeValue>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		FeeChanged(T::FeeKey, BalanceOf<T>),
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the given fee for the key
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_fee())]
		pub fn set_fee(origin: OriginFor<T>, key: T::FeeKey, fee: BalanceOf<T>) -> DispatchResult {
			T::FeeChangeOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;

			<FeeBalances<T>>::insert(key, fee);
			Self::deposit_event(Event::FeeChanged(key, fee));
			Ok(())
		}
	}
}

impl<T: Config, K: EncodeLike<T::FeeKey>> fees::Fees<K> for Pallet<T> {
	type AccountId = T::AccountId;
	type Balance = BalanceOf<T>;

	fn fee_value(key: K) -> BalanceOf<T> {
		<FeeBalances<T>>::get(key)
	}

	fn fee_to_author(from: &Self::AccountId, fee: Fee<BalanceOf<T>, K>) -> DispatchResult {
		if let Some(author) = <pallet_authorship::Pallet<T>>::author() {
			let balance = Self::withdraw_fee(from, fee)?;
			T::Currency::resolve_creating(&author, balance);
		}
		Ok(())
	}

	fn fee_to_burn(from: &Self::AccountId, fee: Fee<BalanceOf<T>, K>) -> DispatchResult {
		Self::withdraw_fee(from, fee).map(|_| ())
	}

	fn fee_to_treasury(from: &Self::AccountId, fee: Fee<BalanceOf<T>, K>) -> DispatchResult {
		let amount = Self::withdraw_fee(from, fee)?;
		T::Treasury::on_unbalanced(amount);
		Ok(())
	}
}

impl<T: Config> Pallet<T> {
	fn withdraw_fee<K: EncodeLike<T::FeeKey>>(
		from: &T::AccountId,
		fee: Fee<BalanceOf<T>, K>,
	) -> Result<ImbalanceOf<T>, DispatchError> {
		let balance = match fee {
			Fee::Balance(balance) => balance,
			Fee::Key(key) => <Self as fees::Fees<K>>::fee_value(key),
		};

		T::Currency::withdraw(
			&from,
			balance,
			WithdrawReasons::FEE.into(),
			ExistenceRequirement::KeepAlive,
		)
	}
}
