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
	pub interest_rate: T::Rate,
	pub normalized_debt: T::Balance,
	pub penalty: T::Rate,
}

impl<T: Config> ActiveInterestRate<T> {
	pub fn activate(interest_rate: T::Rate) -> Result<Self, DispatchError> {
		T::InterestAccrual::reference_rate(interest_rate)?;
		Ok(Self {
			interest_rate,
			normalized_debt: T::Balance::zero(),
			penalty: T::Rate::zero(),
		})
	}

	pub fn deactivate(self) -> Result<T::Rate, DispatchError> {
		T::InterestAccrual::unreference_rate(self.interest_rate)?;
		Ok(self.interest_rate)
	}

	pub fn has_debt(&self) -> bool {
		!self.normalized_debt.is_zero()
	}

	pub fn curent_debt(&self) -> Result<T::Balance, DispatchError> {
		let now = T::Time::now().as_secs();
		T::InterestAccrual::calculate_debt(self.interest_rate, self.normalized_debt, now)
	}

	pub fn current_debt_cached<Cache>(&self, cache: &Cache) -> Result<T::Balance, DispatchError>
	where
		Cache: RateCollection<T::Rate, T::Balance, T::Balance>,
	{
		cache.current_debt(self.interest_rate, self.normalized_debt)
	}

	pub fn adjust_debt(&mut self, adjustment: Adjustment<T::Balance>) -> DispatchResult {
		self.normalized_debt = T::InterestAccrual::adjust_normalized_debt(
			self.interest_rate,
			self.normalized_debt,
			adjustment,
		)?;

		Ok(())
	}

	pub fn set_penalty(&mut self, new_penalty: T::Rate) -> DispatchResult {
		let base_interest_rate = self.interest_rate.ensure_sub(self.penalty)?;
		self.update_interest_rate(base_interest_rate, new_penalty)
	}

	pub fn set_interest_rate(&mut self, base_interest_rate: T::Rate) -> DispatchResult {
		self.update_interest_rate(base_interest_rate, self.penalty)
	}

	fn update_interest_rate(
		&mut self,
		new_base_interest_rate: T::Rate,
		new_penalty: T::Rate,
	) -> DispatchResult {
		let new_interest_rate = new_base_interest_rate.ensure_add(new_penalty)?;
		let old_interest_rate = self.interest_rate;

		T::InterestAccrual::reference_rate(new_interest_rate)?;

		self.normalized_debt = T::InterestAccrual::renormalize_debt(
			old_interest_rate,
			new_interest_rate,
			self.normalized_debt,
		)?;
		self.interest_rate = new_interest_rate;
		self.penalty = new_penalty;

		T::InterestAccrual::unreference_rate(old_interest_rate)
	}
}
