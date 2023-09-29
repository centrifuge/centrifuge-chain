use cfg_primitives::{CollectionId, ItemId, PoolId};
use sp_runtime::{traits::Get, AccountId32};

use crate::{
	util::{self, genesis, MUSD_UNIT},
	Config,
};

const POOL_ADMIN: AccountId32 = util::account(1);
const INVESTOR: AccountId32 = util::account(2);
const BORROWER: AccountId32 = util::account(3);

const POOL_A: PoolId = 23;
const ASSET_A: (CollectionId, ItemId) = (1, ItemId(10));

fn borrow_from_pool<T: Config>() {
	// Creating a pool
	util::give_balance_to::<T>(POOL_ADMIN, T::PoolDeposit::get());
	util::create_pool::<T>(POOL_ADMIN, POOL_A);

	// Funding a pool
	let funds = 100_000 * MUSD_UNIT;
	let tranche_id = util::get::default_tranche_id::<T>(POOL_A);
	util::give_musd_to::<T>(INVESTOR, funds);
	util::give_investor_role::<T>(INVESTOR, POOL_A, tranche_id);
	util::invest::<T>(INVESTOR, POOL_A, tranche_id, funds);
	//util::emulate::advance_time::<T>(T::DefaultMinEpochTime::get());
	//util::close_pool_epoch::<T>(POOL_ADMIN, POOL_A);

	// Borrowing from a pool
	util::give_borrower_role::<T>(BORROWER, POOL_A);
	util::give_asset_to::<T>(BORROWER, ASSET_A);
}

crate::test_with_all_runtimes!(genesis, borrow_from_pool);
