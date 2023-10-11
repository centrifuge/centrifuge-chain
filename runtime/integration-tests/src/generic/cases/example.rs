use cfg_primitives::{Balance, CFG};
use frame_support::traits::Get;

use crate::{
	generic::{
		environment::{Blocks, Env},
		envs::{
			fudge_env::{FudgeEnv, FudgeSupport},
			runtime_env::RuntimeEnv,
		},
		runtime::Runtime,
		utils::genesis::Genesis,
	},
	utils::accounts::Keyring,
};

fn transfer_balance<T: Runtime>() {
	const TRANSFER: Balance = 1000 * CFG;
	const FOR_FEES: Balance = 1 * CFG;

	// Set up all GenesisConfig for your initial state
	// You can choose `RuntimeEnv` by `FudgeEnv` to make it working with fudge
	// environment.
	let mut env = RuntimeEnv::<T>::from_storage(
		Genesis::default()
			.add(pallet_balances::GenesisConfig::<T> {
				balances: vec![(
					Keyring::Alice.to_account_id(),
					T::ExistentialDeposit::get() + FOR_FEES + TRANSFER,
				)],
			})
			.storage(),
	);

	// Call an extrinsic that would be processed immediately
	env.submit(
		Keyring::Alice,
		pallet_balances::Call::transfer {
			dest: Keyring::Bob.into(),
			value: TRANSFER,
		},
	)
	.unwrap();

	// Check for an even occurred in this block
	env.check_event(pallet_balances::Event::Transfer {
		from: Keyring::Alice.to_account_id(),
		to: Keyring::Bob.to_account_id(),
		amount: TRANSFER,
	})
	.unwrap();

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

// Identical to `transfer_balance()` test but using fudge.
fn fudge_transfer_balance<T: Runtime + FudgeSupport>() {
	const TRANSFER: Balance = 1000 * CFG;
	const FOR_FEES: Balance = 1 * CFG;

	let mut env = FudgeEnv::<T>::from_storage(
		Genesis::default()
			.add(pallet_balances::GenesisConfig::<T> {
				balances: vec![(
					Keyring::Alice.to_account_id(),
					T::ExistentialDeposit::get() + FOR_FEES + TRANSFER,
				)],
			})
			.storage(),
	);

	env.submit(
		Keyring::Alice,
		pallet_balances::Call::transfer {
			dest: Keyring::Bob.into(),
			value: TRANSFER,
		},
	)
	.unwrap();

	env.pass(Blocks::ByNumber(1));

	// Check the state
	env.state(|| {
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(Keyring::Bob.to_account_id()),
			TRANSFER
		);
	});

	// Check for an even occurred in this block
	env.check_event(pallet_balances::Event::Transfer {
		from: Keyring::Alice.to_account_id(),
		to: Keyring::Bob.to_account_id(),
		amount: TRANSFER,
	})
	.unwrap();
}

fn call_api<T: Runtime>() {
	let env = RuntimeEnv::<T>::from_storage(Default::default());

	env.state(|| {
		// Call to Core::version() API.
		// It's automatically implemented by the runtime T, so you can easily do:
		// T::version()
		assert_eq!(T::version(), <T as frame_system::Config>::Version::get());
	})
}

fn fudge_call_api<T: Runtime + FudgeSupport>() {
	let env = FudgeEnv::<T>::from_storage(Default::default());

	// Exclusive from fudge environment.
	// It uses a client to access the runtime api.
	env.with_api(|api, latest| {
		// We include the API we want to use
		use sp_api::Core;

		let result = api.version(&latest).unwrap();

		assert_eq!(result, T::version());
		assert_eq!(result, <T as frame_system::Config>::Version::get());
	})
}

fn check_fee<T: Runtime>() {
	let mut env = RuntimeEnv::<T>::from_storage(
		Genesis::default()
			.add(pallet_balances::GenesisConfig::<T> {
				balances: vec![(Keyring::Alice.to_account_id(), 1 * CFG)],
			})
			.storage(),
	);

	env.submit(
		Keyring::Alice,
		frame_system::Call::remark { remark: vec![] },
	)
	.unwrap();

	// Get the fee of the last submitted extrinsic
	let fee = env.last_fee();

	env.state(|| {
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(Keyring::Alice.to_account_id()),
			1 * CFG - fee
		);
	});
}

crate::test_for_runtimes!([development, altair, centrifuge], transfer_balance);
crate::test_for_runtimes!(all, call_api);
crate::test_for_runtimes!(all, check_fee);
crate::test_for_runtimes!(all, fudge_transfer_balance);
crate::test_for_runtimes!(all, fudge_call_api);
