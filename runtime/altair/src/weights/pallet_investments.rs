//! TEMP: This file will be regenerated!

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_investments::WeightInfo for WeightInfo<T> {
	fn update_invest_order() -> Weight {
		Weight::zero()
	}

	fn update_redeem_order() -> Weight {
		Weight::zero()
	}

	fn collect_investments(_: u32) -> Weight {
		Weight::zero()
	}

	fn collect_redemptions(_: u32) -> Weight {
		Weight::zero()
	}
}
