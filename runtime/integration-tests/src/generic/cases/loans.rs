use cfg_primitives::{Balance, CollectionId, ItemId, PoolId, CFG};
use frame_support::traits::Get;

use crate::{
	generic::{
		environment::{Blocks, Env},
		envs::{
			fudge_env::{FudgeEnv, FudgeSupport},
			runtime_env::RuntimeEnv,
		},
		runtime::Runtime,
		utils::{
			self,
			genesis::{
				self,
				currency::{CurrencyInfo, Usd6},
				Genesis,
			},
		},
	},
	utils::accounts::Keyring,
};

const POOL_ADMIN: Keyring = Keyring::Admin;
const INVESTOR: Keyring = Keyring::Alice;
const BORROWER: Keyring = Keyring::Bob;

const FOR_FEES: Balance = 1 * CFG;

const POOL_A: PoolId = 23;
const NFT_A: (CollectionId, ItemId) = (1, ItemId(10));

fn borrow<T: Runtime + FudgeSupport>() {
	let mut env = RuntimeEnv::<T>::from_storage(
		Genesis::<T>::default()
			.add(genesis::balances(T::ExistentialDeposit::get() + FOR_FEES))
			.add(genesis::assets(vec![Usd6::ID]))
			.add(genesis::tokens(vec![(Usd6::ID, Usd6::ED)]))
			.storage(),
	);

	env.state_mut(|| {
		// Creating a pool
		utils::give_balance_to::<T>(POOL_ADMIN.id(), T::PoolDeposit::get());
		utils::create_empty_pool::<T>(POOL_ADMIN.id(), POOL_A, Usd6::ID);

		// Funding a pool
		utils::give_nft_to::<T>(BORROWER.id(), NFT_A);
	});
}

crate::test_for_runtimes!(all, borrow);
