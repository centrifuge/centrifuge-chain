use cfg_types::tokens::{AssetMetadata, CurrencyId};
use frame_support::{assert_ok, dispatch::RawOrigin};
use orml_traits::MultiCurrency;
use staging_xcm::v4::{prelude::XCM_VERSION, Junction::*, Junctions::Here, Location, WeightLimit};

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::fudge_env::{handle::FudgeHandle, FudgeEnv, FudgeRelayRuntime, FudgeSupport},
		utils::{
			currency::{cfg, CurrencyInfo, CustomCurrency},
			genesis,
			genesis::Genesis,
			xcm::{account_location, setup_xcm, transferable_metadata},
		},
	},
	utils::{accounts::Keyring, approx::Approximate},
};

const INITIAL: u32 = 100;
const TRANSFER: u32 = 20;

type Relay<T> = FudgeRelayRuntime<T>;

#[test_runtimes(all)]
fn para_to_sibling_with_foreign_to_foreign_tokens<T: Runtime + FudgeSupport>() {
	let curr = CustomCurrency(CurrencyId::ForeignAsset(1), transferable_metadata::<T>(6));

	let mut env = FudgeEnv::<T>::from_storage(
		Default::default(),
		Genesis::default()
			.add(genesis::tokens::<T>([(curr.id(), curr.val(INITIAL))]))
			.add(genesis::assets::<T>([(curr.id(), &curr.metadata())]))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>([(curr.id(), &curr.metadata())]))
			.storage(),
	);

	setup_xcm(&mut env);

	env.parachain_state_mut(|| {
		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.id()).into(),
			curr.id(),
			curr.val(TRANSFER),
			account_location(1, Some(T::FudgeHandle::SIBLING_ID), Keyring::Bob.id()),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Alice.id()),
			curr.val(INITIAL) - curr.val(TRANSFER)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.sibling_state_mut(|| {
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Bob.id()),
			curr.val(TRANSFER)
		);
	});
}

#[test_runtimes(all)]
fn para_to_sibling_with_native_to_foreign_tokens<T: Runtime + FudgeSupport>() {
	let metadata = transferable_metadata::<T>(18);
	let xnative = CustomCurrency(CurrencyId::ForeignAsset(1), metadata.clone());

	let mut env = FudgeEnv::<T>::from_storage(
		Default::default(),
		Genesis::default()
			.add(genesis::balances::<T>(cfg(INITIAL)))
			.add(genesis::assets::<T>([(CurrencyId::Native, &metadata)]))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>([(xnative.id(), &metadata)]))
			.storage(),
	);

	setup_xcm(&mut env);

	env.parachain_state_mut(|| {
		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.id()).into(),
			CurrencyId::Native,
			cfg(TRANSFER),
			account_location(1, Some(T::FudgeHandle::SIBLING_ID), Keyring::Bob.id()),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.id()),
			cfg(INITIAL) - cfg(TRANSFER)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.sibling_state_mut(|| {
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(xnative.id(), &Keyring::Bob.id()),
			xnative.val(TRANSFER)
		);
	});
}

#[test_runtimes(all)]
fn para_to_sibling_with_foreign_to_native_tokens<T: Runtime + FudgeSupport>() {
	let metadata = transferable_metadata::<T>(18);
	let xnative = CustomCurrency(CurrencyId::ForeignAsset(1), metadata.clone());

	let mut env = FudgeEnv::<T>::from_storage(
		Default::default(),
		Genesis::default()
			.add(genesis::tokens::<T>([(xnative.id(), xnative.val(INITIAL))]))
			.add(genesis::assets::<T>([(xnative.id(), &metadata)]))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>([(CurrencyId::Native, &metadata)]))
			.storage(),
	);

	setup_xcm(&mut env);

	env.parachain_state_mut(|| {
		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.id()).into(),
			xnative.id(),
			xnative.val(TRANSFER),
			account_location(1, Some(T::FudgeHandle::SIBLING_ID), Keyring::Bob.id()),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(xnative.id(), &Keyring::Alice.id()),
			xnative.val(INITIAL) - xnative.val(TRANSFER)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.sibling_state_mut(|| {
		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.id()),
			cfg(TRANSFER)
		);
	});
}

#[test_runtimes(all)]
fn from_to_relay_using_relay_native_tokens<T: Runtime + FudgeSupport>() {
	let xrelay = CustomCurrency(
		CurrencyId::ForeignAsset(1),
		AssetMetadata {
			location: Some(Location::parent().into()),
			..transferable_metadata::<T>(10)
		},
	);

	let mut env = FudgeEnv::<T>::from_storage(
		Genesis::default()
			.add(genesis::balances::<Relay<T>>(xrelay.val(INITIAL)))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>([(xrelay.id(), &xrelay.metadata())]))
			.storage(),
		Default::default(),
	);

	// From Relay to Parachain

	env.relay_state_mut(|| {
		assert_ok!(pallet_xcm::Pallet::<Relay<T>>::force_xcm_version(
			RawOrigin::Root.into(),
			Box::new(Location::new(0, Parachain(T::FudgeHandle::PARA_ID))),
			XCM_VERSION,
		));

		assert_ok!(pallet_xcm::Pallet::<Relay<T>>::reserve_transfer_assets(
			RawOrigin::Signed(Keyring::Alice.id()).into(),
			Box::new(Parachain(T::FudgeHandle::PARA_ID).into()),
			account_location(0, None, Keyring::Bob.id()),
			Box::new((Here, xrelay.val(TRANSFER)).into()),
			0,
		));

		assert_eq!(
			pallet_balances::Pallet::<Relay<T>>::free_balance(&Keyring::Alice.id()),
			(xrelay.val(INITIAL) - xrelay.val(TRANSFER)).approx(0.01)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.parachain_state_mut(|| {
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(xrelay.id(), &Keyring::Bob.id()),
			xrelay.val(TRANSFER)
		);
	});

	// From Parachain to Relay

	env.parachain_state_mut(|| {
		assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
			RawOrigin::Root.into(),
			Box::new(Location::parent()),
			XCM_VERSION,
		));

		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Bob.id()).into(),
			xrelay.id(),
			xrelay.val(TRANSFER / 2),
			account_location(1, None, Keyring::Alice.id()),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(xrelay.id(), &Keyring::Bob.id()),
			(xrelay.val(TRANSFER) - xrelay.val(TRANSFER / 2)).approx(0.01)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.relay_state_mut(|| {
		assert_eq!(
			pallet_balances::Pallet::<Relay<T>>::free_balance(&Keyring::Alice.id()),
			(xrelay.val(INITIAL) - xrelay.val(TRANSFER) + xrelay.val(TRANSFER / 2)).approx(0.01)
		);
	});
}
