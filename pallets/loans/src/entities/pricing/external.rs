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
	traits::{
		EnsureAddAssign, EnsureDiv, EnsureFixedPointNumber, EnsureSub, EnsureSubAssign, One, Zero,
	},
	DispatchError, DispatchResult,
};

use crate::pallet::{Config, Error, PoolIdOf, PriceOf};

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
}

impl<T: Config> ExternalActivePricing<T> {
	pub fn new(info: ExternalPricing<T>, pool_id: PoolIdOf<T>) -> Result<Self, DispatchError> {
		T::PriceRegistry::register_id(&info.price_id, &pool_id)?;
		Ok(Self {
			info,
			outstanding_quantity: T::Balance::zero(),
		})
	}

	pub fn end(self, pool_id: PoolIdOf<T>) -> Result<ExternalPricing<T>, DispatchError> {
		T::PriceRegistry::unregister_id(&self.info.price_id, &pool_id)?;
		Ok(self.info)
	}

	pub fn has_debt(&self) -> bool {
		!self.outstanding_quantity.is_zero()
	}

	pub fn calculate_debt(&self) -> Result<T::Balance, DispatchError> {
		let price = self.calculate_price()?;
		Ok(price.ensure_mul_int(self.outstanding_quantity)?)
	}

	pub fn calculate_price(&self) -> Result<T::Rate, DispatchError> {
		Ok(T::PriceRegistry::get(&self.info.price_id)?.0)
	}

	pub fn calculate_price_by<Prices>(&self, prices: &Prices) -> Result<T::Rate, DispatchError>
	where
		Prices: DataCollection<T::PriceId, Data = Result<PriceOf<T>, DispatchError>>,
	{
		Ok(prices.get(&self.info.price_id)?.0)
	}

	pub fn max_borrow_amount(
		&self,
		desired_amount: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		match self.info.max_borrow_amount {
			MaxBorrowAmount::Quantity(quantity) => {
				let price = self.calculate_price()?;
				let available = quantity.ensure_sub(self.outstanding_quantity)?;
				Ok(price.ensure_mul_int(available)?)
			}
			MaxBorrowAmount::NoLimit => Ok(desired_amount),
		}
	}

	pub fn last_updated(&self) -> Result<Moment, DispatchError> {
		Ok(T::PriceRegistry::get(&self.info.price_id)?.1)
	}

	pub fn compute_present_value(&self, price: T::Rate) -> Result<T::Balance, DispatchError> {
		Ok(price.ensure_mul_int(self.outstanding_quantity)?)
	}

	pub fn adjust_debt(&mut self, adjustment: Adjustment<T::Balance>) -> DispatchResult {
		let price = self.calculate_price()?;
		let amount = adjustment.abs();
		let quantity = T::Rate::one().ensure_div(price)?.ensure_mul_int(amount)?;

		ensure!(
			price.ensure_mul_int(quantity)? == amount,
			Error::<T>::AmountNotMultipleOfPrice
		);

		match adjustment {
			Adjustment::Increase(_) => self.outstanding_quantity.ensure_add_assign(quantity)?,
			Adjustment::Decrease(_) => self.outstanding_quantity.ensure_sub_assign(quantity)?,
		};

		Ok(())
	}
}
