use cfg_primitives::PoolId;

mod setup;

const POOL_A: PoolId = 23;

#[test]
fn loan_lifetime() {
	setup::new_ext().execute_with(|| {
		setup::register_usdt();
		setup::create_pool(POOL_A);
		setup::fund_pool(POOL_A);
	})
}
