use cfg_primitives::{AccountId, Balance, CollectionId, ItemId, PoolId, CFG};
use frame_support::traits::Get;
use orml_traits::GetByKey;

use crate::{
	generic::{
		environment::{Blocks, Env},
		envs::runtime_env::RuntimeEnv,
		runtime::Runtime,
		utils::{
			self,
			genesis::{self, Genesis, MUSD_CURRENCY_ID},
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

fn borrow<T: Runtime>() {
	let mut env = RuntimeEnv::<T>::from_storage(
		Genesis::<T>::default()
			.add(genesis::balances(T::ExistentialDeposit::get() + FOR_FEES))
			.add(genesis::tokens(vec![(
				MUSD_CURRENCY_ID,
				T::ExistentialDeposits::get(&MUSD_CURRENCY_ID),
			)]))
			.add(genesis::assets(vec![MUSD_CURRENCY_ID]))
			.storage(),
	);

	env.state_mut(|| {
		utils::give_balance_to::<T>(POOL_ADMIN.id(), T::PoolDeposit::get());
		utils::give_nft_to::<T>(BORROWER.id(), NFT_A);
	});

	env.state(|| {
		//pallet_uniques::Pallet::<T>::
	});
}
