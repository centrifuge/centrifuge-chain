//! File pending to be auto-generated

use frame_support::weights::Weight;
pub struct WeightInfo<T>(sp_std::marker::PhantomData<T>);
impl<T: frame_system::Config> pallet_liquidity_rewards::WeightInfo for WeightInfo<T> {
	fn on_initialize(_: u32, _: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn stake() -> Weight {
		Weight::zero()
	}

	fn unstake() -> Weight {
		Weight::zero()
	}

	fn claim_reward() -> Weight {
		Weight::zero()
	}

	fn set_distributed_reward() -> Weight {
		Weight::zero()
	}

	fn set_epoch_duration() -> Weight {
		Weight::zero()
	}

	fn set_group_weight() -> Weight {
		Weight::zero()
	}

	fn set_currency_group() -> Weight {
		Weight::zero()
	}
}
