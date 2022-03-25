#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use common_traits::InterestAccrual;
use common_types::{Adjustment, Moment};
use frame_support::traits::UnixTime;
use scale_info::TypeInfo;
use sp_arithmetic::traits::checked_pow;
use sp_runtime::ArithmeticError;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedMul, CheckedSub},
	DispatchError, FixedPointNumber, FixedPointOperand,
};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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
			+ FixedPointNumber
			+ CheckedMul;

		/// The amount type
		type Amount: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ FixedPointOperand
			+ From<Self::NormalizedDebt>
			+ CheckedAdd
			+ CheckedSub
			+ TypeInfo;

		type Time: UnixTime;
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

		/// Emits when the debt adjustment failed
		DebtAdjustmentFailed,

		/// Emits when the interest rate was not used
		NoSuchRate,
	}

	// TODO: add permissionless extrinsic to update any rate

	impl<T: Config> Pallet<T> {
		pub fn get_current_debt(
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
					new_rate.cumulative_rate
				}
				Ok(rate) => {
					let new_cumulative_rate = Self::calculate_cumulative_rate(
						interest_rate_per_sec,
						rate.cumulative_rate,
						rate.last_updated,
					)
					.map_err(|_| Error::<T>::DebtCalculationFailed)?;
					// TODO: this should update the rate

					new_cumulative_rate
				}
			};

			let debt = Self::calculate_debt(normalized_debt, rate)
				.ok_or(Error::<T>::DebtCalculationFailed)?;
			Ok(debt)
		}

		pub fn do_adjust_normalized_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::NormalizedDebt,
			adjustment: Adjustment<T::Amount>,
		) -> Result<T::NormalizedDebt, DispatchError> {
			let rate =
				Rates::<T>::try_get(interest_rate_per_sec).map_err(|_| Error::<T>::NoSuchRate)?;

			let debt = Self::calculate_debt(normalized_debt, rate.cumulative_rate)
				.ok_or(Error::<T>::DebtCalculationFailed)?;

			let new_normalized_debt =
				Self::convert::<T::InterestRate, T::Amount>(rate.cumulative_rate)
					.and_then(|rate| {
						// Apply adjustment to debt
						match adjustment {
							Adjustment::Increase(amount) => debt.checked_add(&amount),
							Adjustment::Decrease(amount) => debt.checked_sub(&amount),
						}
						// Calculate normalized debt = debt / cumulative_rate
						.and_then(|debt| {
							debt.checked_div_int(rate).and_then(|normalized_debt| {
								Self::convert::<T::Amount, T::NormalizedDebt>(normalized_debt)
							})
						})
					})
					.ok_or(Error::<T>::DebtAdjustmentFailed)?;

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
					.and_then(|debt| Self::convert::<T::NormalizedDebt, T::Amount>(debt))
			})
		}

		fn calculate_cumulative_rate<Rate: FixedPointNumber>(
			interest_rate_per_sec: Rate,
			cumulative_rate: Rate,
			last_updated: Moment,
		) -> Result<Rate, DispatchError> {
			// cumulative_rate * interest_rate_per_sec ^ (now - last_updated)
			let time_difference_secs = Self::now()
				.checked_sub(last_updated)
				.ok_or(ArithmeticError::Underflow)?;

			checked_pow(interest_rate_per_sec, time_difference_secs as usize)
				.ok_or(ArithmeticError::Overflow)?
				.checked_mul(&cumulative_rate)
				.ok_or(ArithmeticError::Overflow.into())
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

impl<T: Config> InterestAccrual<T::InterestRate, T::Amount, Adjustment<T::Amount>> for Pallet<T> {
	type NormalizedDebt = T::NormalizedDebt;

	fn current_debt(
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
	) -> Result<T::Amount, DispatchError> {
		Pallet::<T>::get_current_debt(interest_rate_per_sec, normalized_debt)
	}

	fn adjust_normalized_debt(
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
		adjustment: Adjustment<T::Amount>,
	) -> Result<Self::NormalizedDebt, DispatchError> {
		Pallet::<T>::do_adjust_normalized_debt(interest_rate_per_sec, normalized_debt, adjustment)
	}
}
