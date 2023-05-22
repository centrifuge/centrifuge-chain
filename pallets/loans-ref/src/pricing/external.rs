use cfg_primitives::Moment;
use cfg_traits::{
	data::{DataCollection, DataRegistry},
	ops::EnsureMul,
};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{self, RuntimeDebugNoBound};
use scale_info::TypeInfo;
use sp_arithmetic::traits::Saturating;
use sp_runtime::{traits::Zero, DispatchError, DispatchResult};

use crate::pallet::{Config, PoolIdOf, PriceOf};

/// External pricing method
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ExternalPricing<T: Config> {
	/// Id of an external price
	pub price_id: T::PriceId,

	/// Number of items associated to the price id
	pub quantity: T::Balance,
}

impl<T: Config> ExternalPricing<T> {
	pub fn validate(&self) -> DispatchResult {
		T::PriceRegistry::get(&self.price_id).map(|_| ())
	}
}

/// External pricing method with extra attributes for active loans
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ExternalActivePricing<T: Config> {
	/// Basic external pricing info
	info: ExternalPricing<T>,
}

impl<T: Config> ExternalActivePricing<T> {
	pub fn new(info: ExternalPricing<T>, pool_id: PoolIdOf<T>) -> Result<Self, DispatchError> {
		T::PriceRegistry::register_id(&info.price_id, &pool_id)?;
		Ok(Self { info })
	}

	pub fn end(self, pool_id: PoolIdOf<T>) -> Result<ExternalPricing<T>, DispatchError> {
		T::PriceRegistry::unregister_id(&self.info.price_id, &pool_id)?;
		Ok(self.info)
	}

	pub fn calculate_price(&self) -> Result<T::Balance, DispatchError> {
		Ok(T::PriceRegistry::get(&self.info.price_id)?.0)
	}

	pub fn calculate_price_by<Prices>(&self, prices: &Prices) -> Result<T::Balance, DispatchError>
	where
		Prices: DataCollection<T::PriceId, Data = Result<PriceOf<T>, DispatchError>>,
	{
		Ok(prices.get(&self.info.price_id)?.0)
	}

	pub fn last_updated(&self) -> Result<Moment, DispatchError> {
		Ok(T::PriceRegistry::get(&self.info.price_id)?.1)
	}

	pub fn compute_present_value(
		&self,
		price: T::Balance,
		total_repaid: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		if total_repaid.is_zero() {
			Ok(self.info.quantity.ensure_mul(price)?)
		} else {
			Ok(T::Balance::zero())
		}
	}

	pub fn remaining_from(&self, from: T::Balance) -> Result<T::Balance, DispatchError> {
		let price = self.calculate_price()?;
		let total_price = self.info.quantity.ensure_mul(price)?;
		Ok(total_price.saturating_sub(from))
	}
}
