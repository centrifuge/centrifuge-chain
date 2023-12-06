use cfg_types::{
	fixed_point::Rate,
	pools::{FeeAmount, FeeAmountType, FeeBucket},
};
use frame_support::assert_ok;
use sp_arithmetic::FixedPointNumber;

use super::*;
use crate::mock::{
	fees, new_fee, new_test_ext, AccountId, ChangeId, CurrencyId, MockChangeGuard, MockPermissions,
	MockPools, OrmlTokens, PoolFees, Runtime, RuntimeOrigin, ADMIN, ANY, DESTINATION, EDITOR, POOL,
};

fn config_mocks() {
	MockPermissions::mock_add(|_, _, _| Ok(()));
	MockPermissions::mock_has(|_, _, _| true);
	MockPools::mock_pool_exists(|_| true);
	MockPools::mock_account_for(|_| 0);
	MockPools::mock_withdraw(|_, _, _| Ok(()));
	MockPools::mock_deposit(|_, _, _| Ok(()));
	MockPools::mock_bench_create_pool(|_, _| {});
	MockPools::mock_bench_investor_setup(|_, _, _| {});
	MockChangeGuard::mock_note(|_, change| {
		MockChangeGuard::mock_released(move |_, _| Ok(change.clone()));
		Ok(sp_core::H256::default())
	});
}

#[test]
fn propose_new_works() {
	new_test_ext().execute_with(|| {
		let fees = fees();
		config_mocks();

		for (i, fee) in fees.into_iter().enumerate() {
			assert!(
				PoolFees::propose_new_fee(
					RuntimeOrigin::signed(ADMIN),
					POOL,
					FeeBucket::Top,
					fee.clone()
				)
				.is_ok(),
				"Failed to propose fee {:?} at position {:?}",
				fee,
				i
			);
		}
	})
}
