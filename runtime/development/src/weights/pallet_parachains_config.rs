use core::marker::PhantomData;

use pallet_order_book::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> polkadot_runtime_parachains::configuration::WeightInfo
	for WeightInfo<T>
{
	fn set_config_with_block_number() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn set_config_with_u32() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn set_config_with_option_u32() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn set_config_with_balance() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn set_hrmp_open_request_ttl() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn set_config_with_executor_params() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn set_config_with_perbill() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
}
