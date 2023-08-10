use cfg_primitives::Moment;
use cfg_traits::{
	self,
	data::{DataCollection, DataRegistry},
	interest::InterestRate,
};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{self, ensure, RuntimeDebug, RuntimeDebugNoBound};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{EnsureAdd, EnsureFixedPointNumber, EnsureSub, Zero},
	ArithmeticError, DispatchError, DispatchResult, FixedPointNumber,
};

use crate::{
	entities::interest::ActiveInterestRate,
	pallet::{Config, Error, PriceOf},
};

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ExternalAmount<T: Config> {
	pub quantity: T::Quantity,
	pub settlement_price: T::Balance,
}

impl<T: Config> ExternalAmount<T> {
	pub fn new(quantity: T::Quantity, price: T::Balance) -> Self {
		Self {
			quantity,
			settlement_price: price,
		}
	}

	pub fn empty() -> Self {
		Self {
			quantity: T::Quantity::zero(),
			settlement_price: T::Balance::zero(),
		}
	}

	pub fn balance(&self) -> Result<T::Balance, ArithmeticError> {
		self.quantity.ensure_mul_int(self.settlement_price)
	}
}

/// Define the max borrow amount of a loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum MaxBorrowAmount<Quantity> {
	/// You can borrow until the pool reserve
	NoLimit,

	/// Maximum number of items associated with the loan of the pricing.
	Quantity(Quantity),
}

/// External pricing method
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ExternalPricing<T: Config> {
	/// Id of an external price
	pub price_id: T::PriceId,

	/// Maximum amount that can be borrowed.
	pub max_borrow_amount: MaxBorrowAmount<T::Quantity>,

	/// Reference price used to calculate the interest
	pub notional: T::Balance,

	/// Maximum slippage between the settlement price chosen for
	/// borrow/repay and the current oracle price.
	pub slippage: T::Balance,
}

impl<T: Config> ExternalPricing<T> {
	pub fn validate(&self) -> DispatchResult {
		if let MaxBorrowAmount::Quantity(quantity) = self.max_borrow_amount {
			ensure!(
				quantity.frac().is_zero() && quantity >= T::Quantity::zero(),
				Error::<T>::AmountNotNaturalNumber
			)
		}

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
	outstanding_quantity: T::Quantity,

	/// Current interest rate
	pub interest: ActiveInterestRate<T>,
}

impl<T: Config> ExternalActivePricing<T> {
	pub fn activate(
		info: ExternalPricing<T>,
		interest_rate: InterestRate<T::Rate>,
		pool_id: T::PoolId,
	) -> Result<Self, DispatchError> {
		T::PriceRegistry::register_id(&info.price_id, &pool_id)?;
		Ok(Self {
			info,
			outstanding_quantity: T::Quantity::zero(),
			interest: ActiveInterestRate::activate(interest_rate)?,
		})
	}

	pub fn deactivate(
		self,
		pool_id: T::PoolId,
	) -> Result<(ExternalPricing<T>, InterestRate<T::Rate>), DispatchError> {
		T::PriceRegistry::unregister_id(&self.info.price_id, &pool_id)?;
		Ok((self.info, self.interest.deactivate()?))
	}

	pub fn last_updated(&self, pool_id: T::PoolId) -> Result<Moment, DispatchError> {
		Ok(T::PriceRegistry::get(&self.info.price_id, &pool_id)?.1)
	}

	pub fn outstanding_principal(&self, pool_id: T::PoolId) -> Result<T::Balance, DispatchError> {
		let price = T::PriceRegistry::get(&self.info.price_id, &pool_id)?.0;
		Ok(self.outstanding_quantity.ensure_mul_int(price)?)
	}

	pub fn outstanding_interest(&self) -> Result<T::Balance, DispatchError> {
		let outstanding_notional = self
			.outstanding_quantity
			.ensure_mul_int(self.info.notional)?;

		let debt = self.interest.current_debt()?;
		Ok(debt.ensure_sub(outstanding_notional)?)
	}

	pub fn present_value(&self, pool_id: T::PoolId) -> Result<T::Balance, DispatchError> {
		self.outstanding_principal(pool_id)
	}

	pub fn present_value_cached<Prices>(&self, cache: &Prices) -> Result<T::Balance, DispatchError>
	where
		Prices: DataCollection<T::PriceId, Data = PriceOf<T>>,
	{
		let price = cache.get(&self.info.price_id)?.0;
		Ok(self.outstanding_quantity.ensure_mul_int(price)?)
	}

	fn validate_amount(
		&self,
		amount: &ExternalAmount<T>,
		pool_id: T::PoolId,
	) -> Result<(), DispatchError> {
		let price = T::PriceRegistry::get(&self.info.price_id, &pool_id)?.0;
		let variation = if amount.settlement_price > price {
			amount.settlement_price.ensure_sub(price)?
		} else {
			price.ensure_sub(amount.settlement_price)?
		};

		ensure!(
			variation <= self.info.slippage,
			Error::<T>::SettlementPriceExceedsSlippage
		);

		Ok(())
	}

	pub fn max_borrow_amount(
		&self,
		amount: ExternalAmount<T>,
		pool_id: T::PoolId,
	) -> Result<T::Balance, DispatchError> {
		self.validate_amount(&amount, pool_id)?;

		match self.info.max_borrow_amount {
			MaxBorrowAmount::Quantity(quantity) => {
				let available = quantity.ensure_sub(self.outstanding_quantity)?;
				Ok(available.ensure_mul_int(amount.settlement_price)?)
			}
			MaxBorrowAmount::NoLimit => Ok(amount.balance()?),
		}
	}

	pub fn max_repay_principal(
		&self,
		amount: ExternalAmount<T>,
		pool_id: T::PoolId,
	) -> Result<T::Balance, DispatchError> {
		self.validate_amount(&amount, pool_id)?;

		Ok(self
			.outstanding_quantity
			.ensure_mul_int(amount.settlement_price)?)
	}

	pub fn adjust(
		&mut self,
		quantity_adj: Adjustment<T::Quantity>,
		interest: T::Balance,
	) -> DispatchResult {
		self.outstanding_quantity = quantity_adj.ensure_add(self.outstanding_quantity)?;

		let interest_adj = quantity_adj.try_map(|quantity| -> Result<_, DispatchError> {
			ensure!(
				quantity.frac().is_zero() && quantity >= T::Quantity::zero(),
				Error::<T>::AmountNotNaturalNumber
			);

			Ok(quantity
				.ensure_mul_int(self.info.notional)?
				.ensure_add(interest)?)
		})?;

		self.interest.adjust_debt(interest_adj)?;

		Ok(())
	}
}
