use cfg_primitives::{AuraId, Balance, CFG};
use frame_support::{assert_ok, traits::Get};

use crate::{
	generic::{
		env::{self, Blocks, Config, Env},
		envs::runtime_env::RuntimeEnv,
		utils::genesis::Genesis,
	},
	utils::accounts::Keyring,
};

const TRANSFER: Balance = 1000 * CFG;
const FOR_FEES: Balance = 1 * CFG;

fn transfer_balance<T: Config>() {
	// Set up all GenesisConfig for your initial state
	let mut env = RuntimeEnv::<T>::from_genesis(
		Genesis::default()
			.add(pallet_aura::GenesisConfig::<T> {
				authorities: vec![AuraId::from(sp_core::sr25519::Public([0u8; 32]))],
			})
			.add(pallet_balances::GenesisConfig::<T> {
				balances: vec![(
					Keyring::Alice.to_account_id(),
					T::ExistentialDeposit::get() + FOR_FEES + TRANSFER,
				)],
			}),
	);

	// Call an extrinsic that would be processed immediately
	// This call can be called several times in different test places
	assert_ok!(env.submit(
		Keyring::Alice,
		pallet_balances::Call::<T>::transfer {
			dest: Keyring::Bob.into(),
			value: TRANSFER,
		},
	));

	// Check for an even occurred in this block
	assert!(env.has_event(pallet_balances::Event::Transfer {
		from: Keyring::Alice.to_account_id(),
		to: Keyring::Bob.to_account_id(),
		amount: TRANSFER,
	}));

	// Extracting last extrinsic fees
	let fees = env.last_xt_fees();

	// Pass blocks to evolve the system
	env.pass(Blocks::ByNumber(1));

	// Check the state
	// This call can be called several times in different test places
	env.state(|| {
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(Keyring::Alice.to_account_id()),
			T::ExistentialDeposit::get() + FOR_FEES - fees
		);
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(Keyring::Bob.to_account_id()),
			TRANSFER
		);
	});
}

// Generate tests for all runtimes
crate::test_with_all_runtimes!(transfer_balance);
