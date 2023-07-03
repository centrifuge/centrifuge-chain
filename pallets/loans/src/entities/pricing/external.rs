use cfg_primitives::Moment;
use cfg_traits::{
	self,
	data::{DataCollection, DataRegistry},
};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{self, ensure, RuntimeDebug, RuntimeDebugNoBound};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{EnsureDiv, EnsureFixedPointNumber, EnsureSub, One, Zero},
	DispatchError, DispatchResult,
};

use crate::{
	entities::interest::ActiveInterestRate,
	pallet::{Config, Error, PoolIdOf, PriceOf},
};

/// Define the max borrow amount of a loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum MaxBorrowAmount<Balance> {
	/// You can borrow until the pool reserve
	NoLimit,

	/// Maximum number of items associated with the loan of the pricing.
	Quantity(Balance),
}

/// External pricing method
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ExternalPricing<T: Config> {
	/// Id of an external price
	pub price_id: T::PriceId,

	/// Maximum amount that can be borrowed.
	pub max_borrow_amount: MaxBorrowAmount<T::Balance>,

	/// Reference price used to calculate the interest
	pub notional: T::Rate,
}

impl<T: Config> ExternalPricing<T> {
	pub fn validate(&self) -> DispatchResult {
		Ok(())
	}
}

/// External pricing method with extra attributes for active loans
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ExternalActivePricing<T: Config> {
	/// Basic external pricing info
	info: ExternalPricing<T>,

	/// Outstanding quantity that should be repaid.
	outstanding_quantity: T::Balance,

	/// Current interest rate
	pub interest_rate: ActiveInterestRate<T>,
}

impl<T: Config> ExternalActivePricing<T> {
	pub fn activate(
		info: ExternalPricing<T>,
		interest_rate: T::Rate,
		pool_id: PoolIdOf<T>,
	) -> Result<Self, DispatchError> {
		T::PriceRegistry::register_id(&info.price_id, &pool_id)?;
		Ok(Self {
			info,
			outstanding_quantity: T::Balance::zero(),
			interest_rate: ActiveInterestRate::activate(interest_rate)?,
		})
	}

	pub fn deactivate(
		self,
		pool_id: PoolIdOf<T>,
	) -> Result<(ExternalPricing<T>, T::Rate), DispatchError> {
		T::PriceRegistry::unregister_id(&self.info.price_id, &pool_id)?;
		Ok((self.info, self.interest_rate.deactivate()?))
	}

	pub fn current_price(&self) -> Result<T::Rate, DispatchError> {
		Ok(T::PriceRegistry::get(&self.info.price_id)?.0)
	}

	pub fn last_updated(&self) -> Result<Moment, DispatchError> {
		Ok(T::PriceRegistry::get(&self.info.price_id)?.1)
	}

	pub fn outstanding_amount(&self) -> Result<T::Balance, DispatchError> {
		let price = self.current_price()?;
		Ok(price.ensure_mul_int(self.outstanding_quantity)?)
	}

	pub fn current_interest(&self) -> Result<T::Balance, DispatchError> {
		let principal = self
			.info
			.notional
			.ensure_mul_int(self.outstanding_quantity)?;

		Ok(self.interest_rate.current_debt()?.ensure_sub(principal)?)
	}

	pub fn present_value(&self) -> Result<T::Balance, DispatchError> {
		let price = self.current_price()?;
		Ok(price.ensure_mul_int(self.outstanding_quantity)?)
	}

	pub fn present_value_cached<Prices>(&self, cache: &Prices) -> Result<T::Balance, DispatchError>
	where
		Prices: DataCollection<T::PriceId, Data = Result<PriceOf<T>, DispatchError>>,
	{
		let price = cache.get(&self.info.price_id)?.0;
		Ok(price.ensure_mul_int(self.outstanding_quantity)?)
	}

	pub fn max_borrow_amount(
		&self,
		desired_amount: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		match self.info.max_borrow_amount {
			MaxBorrowAmount::Quantity(quantity) => {
				let price = self.current_price()?;
				let available = quantity.ensure_sub(self.outstanding_quantity)?;
				Ok(price.ensure_mul_int(available)?)
			}
			MaxBorrowAmount::NoLimit => Ok(desired_amount),
		}
	}

	pub fn adjust(&mut self, adjustment: Adjustment<T::Balance>) -> DispatchResult {
		let quantity = adjustment.try_map(|amount| -> Result<_, DispatchError> {
			let price = self.current_price()?;
			let quantity = T::Rate::one().ensure_div(price)?.ensure_mul_int(amount)?;

			ensure!(
				price.ensure_mul_int(quantity)? == amount,
				Error::<T>::AmountNotMultipleOfPrice
			);

			Ok(quantity)
		})?;

		self.outstanding_quantity = quantity.ensure_add(self.outstanding_quantity)?;

		self.interest_rate.adjust_debt(
			adjustment.try_map(|quantity| self.info.notional.ensure_mul_int(quantity))?,
		)?;

		Ok(())
	}
}
