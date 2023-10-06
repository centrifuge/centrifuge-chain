use cfg_primitives::{AuraId, Balance, CFG};
use frame_support::{assert_ok, traits::Get};

use crate::{
	generic::{
		environment::{self, Blocks, Env},
		envs::runtime_env::RuntimeEnv,
		runtime::Runtime,
		utils::genesis::Genesis,
	},
	utils::accounts::Keyring,
};

fn transfer_balance<T: Runtime>() {
	const TRANSFER: Balance = 1000 * CFG;
	const FOR_FEES: Balance = 1 * CFG;

	// Set up all GenesisConfig for your initial state
	let mut env = RuntimeEnv::<T>::from_genesis(
		Genesis::default()
			.add(pallet_aura::GenesisConfig::<T> {
				authorities: vec![AuraId::from(Keyring::Charlie.public())],
			})
			.add(pallet_balances::GenesisConfig::<T> {
				balances: vec![(
					Keyring::Alice.to_account_id(),
					T::ExistentialDeposit::get() + FOR_FEES + TRANSFER,
				)],
			}),
	);

	// Call an extrinsic that would be processed immediately
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

	// Pass blocks to evolve the system
	env.pass(Blocks::ByNumber(1));

	// Check the state
	env.state(|| {
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(Keyring::Bob.to_account_id()),
			TRANSFER
		);
	});
}

fn call_api<T: Runtime>() {
	// Set up all GenesisConfig for your initial state
	let mut env =
		RuntimeEnv::<T>::from_genesis(Genesis::default().add(pallet_aura::GenesisConfig::<T> {
			authorities: vec![AuraId::from(Keyring::Charlie.public())],
		}));

	env.state(|| {
		// Call to Core::version() API.
		// It's automatically implemented by the runtime T, so you can easily do:
		// T::version()
		assert_eq!(T::version(), <T as frame_system::Config>::Version::get());
	})
}

// Generate tests for all runtimes
crate::test_for_runtimes!((development, altair, centrifuge), transfer_balance);
crate::test_for_all_runtimes!(call_api);

// Output: for `cargo test -p runtime-integration-tests generic`
// running 6 tests
// test generic::cases::example::call_api::centrifuge ... ok
// test generic::cases::example::call_api::altair ... ok
// test generic::cases::example::call_api::development ... ok
// test generic::cases::example::transfer_balance::altair ... ok
// test generic::cases::example::transfer_balance::development ... ok
// test generic::cases::example::transfer_balance::centrifuge ... ok
