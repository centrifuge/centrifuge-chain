use cfg_primitives::{Balance, CollectionId, ItemId, PoolId};
use cfg_traits::Seconds;
use cfg_types::permissions::PoolRole;
use frame_support::traits::Get;
use runtime_common::apis::runtime_decl_for_PoolsApi::PoolsApiV1;

use crate::{
	generic::{
		environment::{Blocks, Env},
		envs::runtime_env::RuntimeEnv,
		runtime::Runtime,
		utils::{
			self,
			genesis::{
				self,
				currency::{cfg, usd6, CurrencyInfo, Usd6},
				Genesis,
			},
		},
	},
	utils::accounts::Keyring,
};

const POOL_ADMIN: Keyring = Keyring::Admin;
const INVESTOR: Keyring = Keyring::Alice;
const BORROWER: Keyring = Keyring::Bob;

const FOR_FEES: Balance = cfg(1);

const POOL_A: PoolId = 23;
const NFT_A: (CollectionId, ItemId) = (1, ItemId(10));

const EXPECTED_POOL_BALANCE: Balance = usd6(1_000_000);

fn initialize_state_for_loans<Environment: Env<T>, T: Runtime>() -> Environment {
	let mut env = Environment::from_storage(
		Genesis::<T>::default()
			.add(genesis::balances(T::ExistentialDeposit::get() + FOR_FEES))
			.add(genesis::assets(vec![Usd6::ID]))
			.add(genesis::tokens(vec![(Usd6::ID, Usd6::ED)]))
			.storage(),
	);

	env.state_mut(|| {
		// Creating a pool
		utils::give_balance::<T>(POOL_ADMIN.id(), T::PoolDeposit::get());
		utils::create_empty_pool::<T>(POOL_ADMIN.id(), POOL_A, Usd6::ID);

		// Funding a pool
		let tranche_id = T::Api::tranche_id(POOL_A, 0).unwrap();
		let tranche_investor = PoolRole::TrancheInvestor(tranche_id, Seconds::MAX);
		utils::give_pool_role::<T>(INVESTOR.id(), POOL_A, tranche_investor);
		utils::give_tokens::<T>(INVESTOR.id(), Usd6::ID, EXPECTED_POOL_BALANCE);
		utils::invest::<T>(INVESTOR.id(), POOL_A, tranche_id, EXPECTED_POOL_BALANCE);
	});

	env.pass(Blocks::BySeconds(T::DefaultMinEpochTime::get()));

	env.state_mut(|| {
		// New epoch with the investor funds available
		utils::close_pool_epoch::<T>(POOL_ADMIN.id(), POOL_A);

		// Preparing borrower
		utils::give_pool_role::<T>(BORROWER.id(), POOL_A, PoolRole::Borrower);
		utils::give_nft::<T>(BORROWER.id(), NFT_A);
	});

	env
}

fn borrow<T: Runtime>() {
	let mut env = initialize_state_for_loans::<RuntimeEnv<T>, T>();

	// Submit Loan::create()
	// Submit Loan::borrow()
}

crate::test_for_runtimes!(all, borrow);
