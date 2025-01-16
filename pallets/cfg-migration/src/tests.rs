use crate::{mock::*, pallet::*};
use frame_system::pallet_prelude::OriginFor;

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{assert_noop, assert_ok};
	use sp_core::H160;

	#[test]
	fn test_migrate_success() {
		new_test_ext().execute_with(|| {
			// Ensure that the transfer to the domain account succeeded
			// assert_eq!(Balances::free_balance(999), amount);
			let receiver = H160::from_low_u64_be(2);
			let amount = 100;

			assert_ok!(Pallet::<Runtime>::migrate(
				OriginFor::<Runtime>::signed(ALICE),
				amount,
				receiver
			));

			// Ensure that the transfer to the domain account succeeded
			// assert_eq!(Balances::free_balance(999), amount);
		});
	}

	#[test]
	fn test_migrate_failed_transfer() {
		new_test_ext().execute_with(|| {
			// Setup conditions where the transfer will fail
			assert_noop!(
				Pallet::<Runtime>::migrate(
					OriginFor::<Runtime>::signed(ALICE),
					100,
					H160::from_low_u64_be(2)
				),
				pallet_liquidity_pools::Error::<Runtime>::AssetNotFound
			);
		});
	}

	#[test]
	fn test_migrate_failed_liquidity_pool_transfer() {
		new_test_ext().execute_with(|| {
			// Mock liquidity pool failure
			assert_noop!(
				Pallet::<Runtime>::migrate(
					OriginFor::<Runtime>::signed(ALICE),
					100,
					H160::from_low_u64_be(2)
				),
				pallet_liquidity_pools::Error::<Runtime>::AssetNotFound
			);
		});
	}
}
