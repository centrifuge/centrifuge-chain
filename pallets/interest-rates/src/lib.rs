#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use common_traits::{InterestRates};
use sp_runtime::{
	traits::{
		AtLeast32BitUnsigned,
	},
	DispatchError, FixedPointNumber, FixedPointOperand
};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Type that indicates a point in time
type Moment = u64;

pub enum Adjustment<Amount: FixedPointNumber> {
	Increase(Amount),
	Decrease(Amount),
}

// Type aliases
type RateDetailsOf<T> = RateDetails<
	<T as Config>::InterestRate,
	Moment
>;

// Storage types
#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct RateDetails<InterestRate, Moment> {
	// chi in MCD Rates
	pub cumulative_rate: InterestRate,

	// when cumulative_rate was last updated
	pub last_updated: Moment,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::{PalletId};

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u64>
			+ From<u128>
			+ TypeInfo
			+ TryInto<u64>;

		/// A fixed-point number which represents
		/// an interest rate.
		type InterestRate: Member
			+ Parameter
			+ Default
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>;

		/// A fixed-point number which represents
		/// the normalized debt.
		type NormalizedDebt: Member
			+ Parameter
			+ Default
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>;

		/// The amount type
		type Amount: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ From<u64>
			+ TypeInfo;

		/// The Id of this pallet
		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	#[pallet::storage]
	#[pallet::getter(fn get_sale)]
	pub(super) type Rates<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::InterestRate,
		RateDetailsOf<T>,
	>;

	#[pallet::event]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	impl<T: Config> Pallet<T> {}
}


impl<T: Config> InterestRates<T::InterestRate, T::Amount> for Pallet<T> {
	type NormalizedDebt = T::NormalizedDebt;
	type Adjustment = Adjustment<T::Amount>;

	fn get_current_debt(interest_rate_per_sec: T::InterestRate, normalized_debt: Self::NormalizedDebt) -> Result<T::Amount, DispatchError> {
		let current_debt: T::Amount = 0u64.into();
		Ok(current_debt)
	}

	fn adjust_normalized_debt(interest_rate_per_sec: T::InterestRate, normalized_debt: Self::NormalizedDebt, adjustment: Self::Adjustment) -> Result<T::Amount, DispatchError> {
		let new_normalized_debt: T::Amount = 0u64.into();
		Ok(new_normalized_debt)
	}
}
