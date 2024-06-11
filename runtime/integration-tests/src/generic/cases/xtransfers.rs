use cfg_types::tokens::CurrencyId;
use frame_support::{assert_ok, dispatch::RawOrigin};
use orml_traits::MultiCurrency;
use staging_xcm::v4::WeightLimit;

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::fudge_env::{handle::FudgeHandle, FudgeEnv, FudgeSupport},
		utils::{
			currency::{cfg, CurrencyInfo, CustomCurrency},
			genesis,
			genesis::Genesis,
			xcm::{account_location, setup_xcm, transferable_metadata},
		},
	},
	utils::accounts::Keyring,
};

const INITIAL: u32 = 100;
const TRANSFER: u32 = 80;

#[test_runtimes(all)]
fn foreign_to_foreign_tokens<T: Runtime + FudgeSupport>() {
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
			account_location(Keyring::Bob.id(), T::FudgeHandle::SIBLING_ID),
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
fn native_to_foreign_tokens<T: Runtime + FudgeSupport>() {
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
			account_location(Keyring::Bob.id(), T::FudgeHandle::SIBLING_ID),
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
fn foreign_to_native_tokens<T: Runtime + FudgeSupport>() {
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
			account_location(Keyring::Bob.id(), T::FudgeHandle::SIBLING_ID),
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
