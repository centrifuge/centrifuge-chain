use cfg_primitives::PoolId;
use sp_runtime::{traits::Get, AccountId32};

use crate::{
	util::{self, genesis, Account, MUSD_UNIT},
	Config,
};

const POOL_A: PoolId = 23;

const AAA: AccountId32 = AccountId32::new([0; 32]);

const ADMIN: Account = Account(1);
const INVESTOR: Account = Account(2);
const BORROWER: Account = Account(3);

fn borrow_from_pool<T: Config>() {
	// Creating a pool
	util::give_balance_to::<T>(ADMIN, T::PoolDeposit::get());
	util::create_pool::<T>(ADMIN, POOL_A);

	// Funding a pool
	let funds = 100_000 * MUSD_UNIT;
	let tranche_id = util::get::default_tranche_id::<T>(POOL_A);
	util::give_musd_to::<T>(INVESTOR, funds);
	util::give_investor_role::<T>(INVESTOR, POOL_A, tranche_id);
	util::invest::<T>(INVESTOR, POOL_A, tranche_id, funds);

	// Borrowing from a pool
	util::give_borrower_role::<T>(BORROWER, POOL_A);
}

crate::test_with_all_runtimes!(genesis, borrow_from_pool);
