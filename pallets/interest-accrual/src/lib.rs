#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use common_traits::InterestAccrual;
use frame_support::traits::UnixTime;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedMul},
	DispatchError, FixedPointNumber, FixedPointOperand,
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
type RateDetailsOf<T> = RateDetails<<T as Config>::InterestRate, Moment>;

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
	use frame_support::PalletId;

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
			+ CheckedMul
			+ FixedPointNumber<Inner = Self::Amount>;

		/// The amount type
		type Amount: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ From<u64>
			+ From<Self::NormalizedDebt>
			+ TypeInfo;

		type Time: UnixTime;

		/// The Id of this pallet
		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	#[pallet::storage]
	#[pallet::getter(fn get_sale)]
	pub(super) type Rates<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InterestRate, RateDetailsOf<T>, OptionQuery>;

	#[pallet::event]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when the debt calculation failed
		DebtCalculationFailed,
	}

	// TODO: add permissionless extrinsic to update any rate

	impl<T: Config> Pallet<T> {
		pub fn do_get_current_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::NormalizedDebt,
		) -> Result<T::Amount, DispatchError> {
			let rate = match Rates::<T>::try_get(interest_rate_per_sec) {
				Err(_) => {
					let new_rate = RateDetails {
						cumulative_rate: T::InterestRate::saturating_from_rational(100, 100).into(),
						last_updated: Self::now(),
					};
					Rates::<T>::insert(interest_rate_per_sec, &new_rate);
					new_rate
				}
				Ok(rate) => {
					// TODO: this should update the rate
					rate
				}
			};

			let debt = Self::calculate_debt(normalized_debt, rate.cumulative_rate)
				.ok_or(Error::<T>::DebtCalculationFailed)?;
			Ok(debt)
		}

		pub fn do_adjust_normalized_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::NormalizedDebt,
			adjustment: Adjustment<T::Amount>,
		) -> Result<T::Amount, DispatchError> {
			let new_normalized_debt: T::Amount = 0u64.into();
			Ok(new_normalized_debt)
		}

		/// Calculates the debt using debt = normalized_debt * cumulative_rate
		fn calculate_debt(
			normalized_debt: T::NormalizedDebt,
			cumulative_rate: T::InterestRate,
		) -> Option<T::Amount> {
			// TODO: isn't there a better way of doing this, without the convert?
			Self::convert::<T::InterestRate, T::NormalizedDebt>(cumulative_rate).and_then(|rate| {
				normalized_debt
					.checked_mul(&rate)
					.and_then(|debt| Some(debt.into()))
			})
		}

		/// converts a fixed point from A precision to B precision
		/// we don't convert from un-signed to signed or vice-verse
		fn convert<A: FixedPointNumber, B: FixedPointNumber>(a: A) -> Option<B> {
			if A::SIGNED != B::SIGNED {
				return None;
			}

			B::checked_from_rational(a.into_inner(), A::accuracy())
		}

		fn now() -> Moment {
			T::Time::now().as_secs()
		}
	}
}

impl<T: Config> InterestAccrual<T::InterestRate, T::Amount> for Pallet<T> {
	type NormalizedDebt = T::NormalizedDebt;
	type Adjustment = Adjustment<T::Amount>;

	fn get_current_debt(
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
	) -> Result<T::Amount, DispatchError> {
		Pallet::<T>::do_get_current_debt(interest_rate_per_sec, normalized_debt)
	}

	fn adjust_normalized_debt(
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
		adjustment: Self::Adjustment,
	) -> Result<T::Amount, DispatchError> {
		Pallet::<T>::do_adjust_normalized_debt(interest_rate_per_sec, normalized_debt, adjustment)
	}
}
