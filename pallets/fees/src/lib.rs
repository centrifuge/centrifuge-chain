//! # Fees pallet for runtime
//!
//! This pallet provides a storing functionality for setting and getting fees
//! associated with an Hash key. Fees can only be set by FeeOrigin or RootOrigin
//!
//! Also, for its internal usage from the runtime or other pallets,
//! it offers some utilities to transfer the fees to the author, the treasury or
//! burn it.
#![cfg_attr(not(feature = "std"), no_std)]

use cfg_traits::fees::{self, Fee, FeeKey};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	traits::{Currency, ExistenceRequirement, Imbalance, OnUnbalanced, WithdrawReasons},
};
pub use pallet::*;
use parity_scale_codec::EncodeLike;
use sp_std::vec::Vec;

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
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use super::*;

	// Simple declaration of the `Pallet` type. It is placeholder we use to
	// implement traits and method.
	#[pallet::pallet]

	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_authorship::Config {
		/// Key type used for storing and identifying fees.
		type FeeKey: FeeKey + EncodeLike + MaxEncodedLen;

		/// The currency mechanism.
		type Currency: Currency<Self::AccountId>;

		/// The treasury destination.
		type Treasury: OnUnbalanced<ImbalanceOf<Self>>;

		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Required origin for changing fees.
		type FeeChangeOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Default value for fee keys.
		type DefaultFeeValue: Get<BalanceOf<Self>>;

		/// Type representing the weight of this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Stores the fee balances associated with a Hash identifier
	#[pallet::storage]
	#[pallet::getter(fn fee)]
	pub(super) type FeeBalances<T: Config> =
		StorageMap<_, Blake2_256, T::FeeKey, BalanceOf<T>, ValueQuery, T::DefaultFeeValue>;

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

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		FeeChanged {
			key: T::FeeKey,
			fee: BalanceOf<T>,
		},
		FeeToAuthor {
			from: T::AccountId,
			balance: BalanceOf<T>,
		},
		FeeToBurn {
			from: T::AccountId,
			balance: BalanceOf<T>,
		},
		FeeToTreasury {
			from: T::AccountId,
			balance: BalanceOf<T>,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the given fee for the key
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_fee())]
		#[pallet::call_index(0)]
		pub fn set_fee(origin: OriginFor<T>, key: T::FeeKey, fee: BalanceOf<T>) -> DispatchResult {
			T::FeeChangeOrigin::ensure_origin(origin)?;

			<FeeBalances<T>>::insert(key.clone(), fee);
			Self::deposit_event(Event::FeeChanged { key, fee });

			Ok(())
		}
	}
}

impl<T: Config> fees::Fees for Pallet<T> {
	type AccountId = T::AccountId;
	type Balance = BalanceOf<T>;
	type FeeKey = T::FeeKey;

	fn fee_value(key: Self::FeeKey) -> BalanceOf<T> {
		<FeeBalances<T>>::get(key)
	}

	fn fee_to_author(
		from: &Self::AccountId,
		fee: Fee<BalanceOf<T>, Self::FeeKey>,
	) -> DispatchResult {
		if let Some(author) = <pallet_authorship::Pallet<T>>::author() {
			let imbalance = Self::withdraw_fee(from, fee)?;
			let balance = imbalance.peek();

			T::Currency::resolve_creating(&author, imbalance);

			Self::deposit_event(Event::FeeToAuthor {
				from: author,
				balance,
			});
		}
		Ok(())
	}

	fn fee_to_burn(from: &Self::AccountId, fee: Fee<BalanceOf<T>, Self::FeeKey>) -> DispatchResult {
		let imbalance = Self::withdraw_fee(from, fee)?;
		let balance = imbalance.peek();

		Self::deposit_event(Event::FeeToBurn {
			from: from.clone(),
			balance,
		});
		Ok(())
	}

	fn fee_to_treasury(
		from: &Self::AccountId,
		fee: Fee<BalanceOf<T>, Self::FeeKey>,
	) -> DispatchResult {
		let imbalance = Self::withdraw_fee(from, fee)?;
		let balance = imbalance.peek();

		T::Treasury::on_unbalanced(imbalance);

		Self::deposit_event(Event::FeeToTreasury {
			from: from.clone(),
			balance,
		});
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn add_fee_requirements(from: &Self::AccountId, fee: Fee<Self::Balance, Self::FeeKey>) {
		T::Currency::deposit_creating(from, T::Currency::minimum_balance());
		T::Currency::deposit_creating(from, fee.value::<Self>());
	}
}

impl<T: Config> Pallet<T> {
	fn withdraw_fee(
		from: &T::AccountId,
		fee: Fee<BalanceOf<T>, T::FeeKey>,
	) -> Result<ImbalanceOf<T>, DispatchError> {
		T::Currency::withdraw(
			from,
			fee.value::<Self>(),
			WithdrawReasons::FEE,
			ExistenceRequirement::KeepAlive,
		)
	}
}
