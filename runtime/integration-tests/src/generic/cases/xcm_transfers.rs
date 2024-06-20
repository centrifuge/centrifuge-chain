use cfg_types::tokens::{AssetMetadata, CurrencyId, CurrencyId::Native};
use frame_support::{assert_ok, dispatch::RawOrigin};
use orml_traits::MultiCurrency;
use staging_xcm::v4::{Junction::*, Junctions::Here, WeightLimit};

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::fudge_env::{
			handle::{PARA_ID, SIBLING_ID},
			FudgeEnv, FudgeSupport, RelayRuntime,
		},
		utils::{
			currency::{cfg, CurrencyInfo, CustomCurrency},
			genesis,
			genesis::Genesis,
			xcm::{
				account_location, enable_para_to_relay_communication,
				enable_para_to_sibling_communication, enable_relay_to_para_communication,
				transferable_metadata,
			},
		},
	},
	utils::{accounts::Keyring, approx::Approximate},
};

const INITIAL: u32 = 100;
const TRANSFER: u32 = 20;

fn create_transfeable_currency(decimals: u32, para_id: Option<u32>) -> CustomCurrency {
	CustomCurrency(
		CurrencyId::ForeignAsset(1),
		AssetMetadata {
			decimals,
			..transferable_metadata(para_id)
		},
	)
}

#[test_runtimes(all)]
fn para_to_sibling_with_foreign_to_foreign_tokens<T: Runtime + FudgeSupport>() {
	let curr = create_transfeable_currency(6, Some(PARA_ID));

	let mut env = FudgeEnv::<T>::from_storage(
		Default::default(),
		Genesis::default()
			.add(genesis::tokens::<T>([(curr.id(), curr.val(INITIAL))]))
			.add(genesis::assets::<T>([(curr.id(), curr.metadata())]))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>([(curr.id(), curr.metadata())]))
			.storage(),
	);

	enable_para_to_sibling_communication::<T>(&mut env);

	env.parachain_state_mut(|| {
		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.id()).into(),
			curr.id(),
			curr.val(TRANSFER),
			account_location(1, Some(SIBLING_ID), Keyring::Bob.id()),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Alice.id()),
			curr.val(INITIAL) - curr.val(TRANSFER)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.sibling_state(|| {
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Bob.id()),
			curr.val(TRANSFER)
		);
	});
}

#[test_runtimes(all)]
fn para_to_sibling_with_native_to_foreign_tokens<T: Runtime + FudgeSupport>() {
	let curr = create_transfeable_currency(18, Some(PARA_ID));

	let mut env = FudgeEnv::<T>::from_storage(
		Default::default(),
		Genesis::default()
			.add(genesis::balances::<T>(cfg(INITIAL)))
			.add(genesis::assets::<T>([(Native, curr.metadata())]))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>([(curr.id(), curr.metadata())]))
			.storage(),
	);

	enable_para_to_sibling_communication::<T>(&mut env);

	env.parachain_state_mut(|| {
		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.id()).into(),
			Native,
			cfg(TRANSFER),
			account_location(1, Some(SIBLING_ID), Keyring::Bob.id()),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.id()),
			cfg(INITIAL) - cfg(TRANSFER)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.sibling_state(|| {
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Bob.id()),
			curr.val(TRANSFER)
		);
	});
}

#[test_runtimes(all)]
fn para_to_sibling_with_foreign_to_native_tokens<T: Runtime + FudgeSupport>() {
	let curr = create_transfeable_currency(18, Some(PARA_ID));

	let mut env = FudgeEnv::<T>::from_storage(
		Default::default(),
		Genesis::default()
			.add(genesis::tokens::<T>([(curr.id(), curr.val(INITIAL))]))
			.add(genesis::assets::<T>([(curr.id(), curr.metadata())]))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>([(Native, curr.metadata())]))
			.storage(),
	);

	enable_para_to_sibling_communication::<T>(&mut env);

	env.parachain_state_mut(|| {
		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.id()).into(),
			curr.id(),
			curr.val(TRANSFER),
			account_location(1, Some(SIBLING_ID), Keyring::Bob.id()),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Alice.id()),
			curr.val(INITIAL) - curr.val(TRANSFER)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.sibling_state(|| {
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.id()),
			cfg(TRANSFER)
		);
	});
}

#[test_runtimes(all)]
fn para_from_to_relay_using_relay_native_tokens<T: Runtime + FudgeSupport>() {
	let curr = create_transfeable_currency(10, None);

	let mut env = FudgeEnv::<T>::from_storage(
		Genesis::default()
			.add(genesis::balances::<RelayRuntime<T>>(curr.val(INITIAL)))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>([(curr.id(), curr.metadata())]))
			.storage(),
		Default::default(),
	);

	// From Relay to Parachain
	enable_relay_to_para_communication::<T>(&mut env);

	env.relay_state_mut(|| {
		assert_ok!(
			pallet_xcm::Pallet::<RelayRuntime<T>>::reserve_transfer_assets(
				RawOrigin::Signed(Keyring::Alice.id()).into(),
				Box::new(Parachain(PARA_ID).into()),
				account_location(0, None, Keyring::Bob.id()),
				Box::new((Here, curr.val(TRANSFER)).into()),
				0,
			)
		);

		assert_eq!(
			pallet_balances::Pallet::<RelayRuntime<T>>::free_balance(&Keyring::Alice.id()),
			(curr.val(INITIAL) - curr.val(TRANSFER)).approx(0.01)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.parachain_state(|| {
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Bob.id()),
			curr.val(TRANSFER)
		);
	});

	// From Parachain to Relay
	enable_para_to_relay_communication::<T>(&mut env);

	env.parachain_state_mut(|| {
		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Bob.id()).into(),
			curr.id(),
			curr.val(TRANSFER / 2),
			account_location(1, None, Keyring::Alice.id()),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Bob.id()),
			(curr.val(TRANSFER) - curr.val(TRANSFER / 2)).approx(0.01)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.relay_state(|| {
		assert_eq!(
			pallet_balances::Pallet::<RelayRuntime<T>>::free_balance(&Keyring::Alice.id()),
			(curr.val(INITIAL) - curr.val(TRANSFER) + curr.val(TRANSFER / 2)).approx(0.01)
		);
	});
}
