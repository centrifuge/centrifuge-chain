use cfg_primitives::PoolId;

mod generic;
mod setup;

use generic::Config;
use setup::new_ext;

const POOL_A: PoolId = 23;

fn loan_lifetime<T: Config>() {
	setup::register_usdt::<T>();
	setup::create_pool::<T>(POOL_A);
	//setup::<T>::fund_pool::<T>(POOL_A);

	// ... actual loan testing part
}

crate::test_for_all_runtimes!(new_ext, loan_lifetime);
