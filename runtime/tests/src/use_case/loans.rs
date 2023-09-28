use cfg_primitives::PoolId;

use crate::{
	setup::{self, new_ext},
	Config,
};

const POOL_A: PoolId = 23;

fn loan_lifetime<T: Config>() {
	setup::register_musd::<T>();
	setup::create_pool::<T>(POOL_A);
	setup::fund_pool::<T>(POOL_A);

	// ... actual loan testing part
}

crate::test_with_all_runtimes!(new_ext, loan_lifetime);
