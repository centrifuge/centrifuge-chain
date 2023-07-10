use cfg_traits::{InterestAccrual, RateCollection};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::UnixTime, RuntimeDebugNoBound};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{EnsureAdd, EnsureSub, Zero},
	DispatchError, DispatchResult,
};

use crate::pallet::Config;

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ActiveInterestRate<T: Config> {
	/// The current interest rate value (per year).
	/// It the rate it has been penalized,
	/// it contains the result of applying that penalty:
	/// rate = base_rate + penalty
	rate: T::Rate,

	/// Normalized accumulation of the interest rate.
	/// Used to get the current interest per second
	normalized_acc: T::Balance,

	/// Penalty applied to this interest rate
	penalty: T::Rate,
}

impl<T: Config> ActiveInterestRate<T> {
	pub fn activate(interest_rate: T::Rate) -> Result<Self, DispatchError> {
		T::InterestAccrual::reference_rate(interest_rate)?;
		Ok(Self {
			rate: interest_rate,
			normalized_acc: T::Balance::zero(),
			penalty: T::Rate::zero(),
		})
	}

	pub fn deactivate(self) -> Result<T::Rate, DispatchError> {
		T::InterestAccrual::unreference_rate(self.rate)?;
		Ok(self.rate)
	}

	pub fn has_debt(&self) -> bool {
		!self.normalized_acc.is_zero()
	}

	pub fn rate(&self) -> T::Rate {
		self.rate
	}

	pub fn penalty(&self) -> T::Rate {
		self.penalty
	}

	pub fn current_debt(&self) -> Result<T::Balance, DispatchError> {
		let now = T::Time::now().as_secs();
		T::InterestAccrual::calculate_debt(self.rate, self.normalized_acc, now)
	}

	pub fn current_debt_cached<Rates>(&self, cache: &Rates) -> Result<T::Balance, DispatchError>
	where
		Rates: RateCollection<T::Rate, T::Balance, T::Balance>,
	{
		cache.current_debt(self.rate, self.normalized_acc)
	}

	pub fn adjust_debt(&mut self, adjustment: Adjustment<T::Balance>) -> DispatchResult {
		self.normalized_acc =
			T::InterestAccrual::adjust_normalized_debt(self.rate, self.normalized_acc, adjustment)?;

		Ok(())
	}

	pub fn set_penalty(&mut self, new_penalty: T::Rate) -> DispatchResult {
		let base_rate = self.rate.ensure_sub(self.penalty)?;
		self.update_rate(base_rate, new_penalty)
	}

	pub fn set_base_rate(&mut self, base_rate: T::Rate) -> DispatchResult {
		self.update_rate(base_rate, self.penalty)
	}

	fn update_rate(&mut self, new_base_rate: T::Rate, new_penalty: T::Rate) -> DispatchResult {
		let new_rate = new_base_rate.ensure_add(new_penalty)?;
		let old_rate = self.rate;

		T::InterestAccrual::reference_rate(new_rate)?;

		self.normalized_acc =
			T::InterestAccrual::renormalize_debt(old_rate, new_rate, self.normalized_acc)?;
		self.rate = new_rate;
		self.penalty = new_penalty;

		T::InterestAccrual::unreference_rate(old_rate)
	}
}
