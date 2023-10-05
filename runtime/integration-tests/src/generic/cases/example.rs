use frame_support::assert_ok;

use crate::{
	generic::{
		env::{self, Config, Env},
		envs::runtime_env::RuntimeEnv,
	},
	utils::accounts::Keyring,
};

fn roundtrip_alice_bob<T: Config>() {
	/*
	let genesis = Genesis::new()
		.add(pallet_balances::GenesisConfig::<T> {
			balances: (0..MAX_FUNDED_ACCOUNTS)
				.into_iter()
				.map(|i| (account(i), T::ExistentialDeposit::get()))
				.collect(),
			}
		);
		.add(orml_tokens::GenesisConfig::<T> {
			balances: (0..MAX_FUNDED_ACCOUNTS)
				.into_iter()
				.map(|i| (account(i), MUSD_CURRENCY_ID, T::ExistentialDeposit::get()))
				.collect(),
		})
	*/

	let mut env = RuntimeEnv::<T>::empty();

	assert_ok!(env.submit(
		Keyring::Alice,
		pallet_balances::Call::<T>::transfer {
			dest: Keyring::Bob.into(),
			value: 1000,
		},
	));

	// Make the submitted extrinsics effective by computing a block.
	env.pass(1);

	/*
	env.state(|| {
		assert_eq!(pallet_balances::Pallet::<T>::total_issuance(), 1);
	});
	*/

	assert_ok!(env.submit(
		Keyring::Alice,
		pallet_balances::Call::<T>::transfer {
			dest: Keyring::Alice.into(),
			value: 1000,
		},
	));

	env.pass(1);

	env.state(|| {});
}

#[test]
fn test_roundtrip_alice_bob() {
	roundtrip_alice_bob::<development_runtime::Runtime>();
}
