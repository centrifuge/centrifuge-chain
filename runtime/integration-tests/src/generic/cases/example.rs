use std::ptr::hash;

use cfg_primitives::{Balance, CFG};
use frame_support::traits::Get;
use sp_api::runtime_decl_for_core::CoreV4;

use crate::{
	chain::centrifuge,
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
	let fee = env
		.submit_now(
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

	// Check the state
	env.state(|| {
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(Keyring::Alice.to_account_id()),
			T::ExistentialDeposit::get() + FOR_FEES - fee,
		);
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(Keyring::Bob.to_account_id()),
			TRANSFER
		);
	});

	// Pass blocks to evolve the system
	env.pass(Blocks::ByNumber(1));
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

	env.submit_later(
		Keyring::Alice,
		pallet_balances::Call::transfer {
			dest: Keyring::Bob.into(),
			value: TRANSFER,
		},
	)
	.unwrap();

	// submit-later will only take effect if a block has passed
	env.pass(Blocks::ByNumber(1));

	// Check for an even occurred in this block
	env.check_event(pallet_balances::Event::Transfer {
		from: Keyring::Alice.to_account_id(),
		to: Keyring::Bob.to_account_id(),
		amount: TRANSFER,
	})
	.unwrap();

	// Look for the fee for the last transaction
	let fee = env
		.find_event(|e| match e {
			pallet_transaction_payment::Event::TransactionFeePaid { actual_fee, .. } => {
				Some(actual_fee)
			}
			_ => None,
		})
		.unwrap();

	// Check the state
	env.state(|| {
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(Keyring::Alice.to_account_id()),
			T::ExistentialDeposit::get() + FOR_FEES - fee,
		);
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(Keyring::Bob.to_account_id()),
			TRANSFER
		);
	});
}

fn call_api<T: Runtime>() {
	let env = RuntimeEnv::<T>::from_storage(Default::default());

	env.state(|| {
		// If imported the trait: sp_api::runtime_decl_for_core::CoreV4,
		// you can easily do: T::Api::version()
		assert_eq!(
			T::Api::version(),
			<T as frame_system::Config>::Version::get()
		);
	})
}

fn fudge_call_api<T: Runtime + FudgeSupport>() {
	let env = FudgeEnv::<T>::from_storage(Default::default());

	// Exclusive from fudge environment.
	// It uses a client to access the runtime api.
	env.with_api(|api, latest| {
		// We include the API we want to use
		use sp_api::Core;

		let hash = match latest {
			sp_runtime::generic::BlockId::<T::Block>::Hash(hash) => hash,
			sp_runtime::generic::BlockId::<T::Block>::Number(n) => {
				todo!("convert block number into H256 hash")
			}
		};

		let result = api.version(hash).unwrap();

		assert_eq!(result, T::Api::version());
		assert_eq!(result, <T as frame_system::Config>::Version::get());
	})
}

crate::test_for_runtimes!([development, altair, centrifuge], transfer_balance);
crate::test_for_runtimes!(all, call_api);
crate::test_for_runtimes!(all, fudge_transfer_balance);
crate::test_for_runtimes!(all, fudge_call_api);
