use cfg_types::tokens::{CrossChainTransferability, CurrencyId, CustomMetadata};
use frame_support::{assert_ok, dispatch::RawOrigin};
use orml_traits::MultiCurrency;
use staging_xcm::{
	v4::{Junction, Junction::*, Location, WeightLimit},
	VersionedLocation,
};

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::fudge_env::{handle::FudgeHandle, FudgeEnv, FudgeSupport},
		utils::{
			currency::{cfg, CurrencyInfo, CustomCurrency},
			genesis,
			genesis::Genesis,
			xcm::setup_xcm,
		},
	},
	utils::accounts::Keyring,
};

fn bob_in_sibling<T: FudgeSupport>() -> Box<VersionedLocation> {
	Box::new(
		Location::new(
			1,
			[
				Parachain(T::FudgeHandle::SIBLING_ID),
				Junction::from(Keyring::Bob.bytes()),
			],
		)
		.into(),
	)
}

#[test_runtimes(all)]
fn transfer_native_tokens_to_sibling<T: Runtime + FudgeSupport>() {
	let native_curr = CustomCurrency {
		id: CurrencyId::Native,
		decimals: 18,
		location: Location::new(1, Parachain(T::FudgeHandle::PARA_ID)),
		custom: CustomMetadata {
			transferability: CrossChainTransferability::xcm_with_fees(0),
			..Default::default()
		},
	};

	let xnative_curr = CustomCurrency {
		id: CurrencyId::ForeignAsset(99),
		..native_curr.clone()
	};

	let mut env = FudgeEnv::<T>::from_storage(
		Default::default(),
		Genesis::default()
			.add(genesis::balances::<T>(cfg(100)))
			.add(genesis::assets::<T>(vec![&native_curr]))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>(vec![&xnative_curr]))
			.storage(),
	);

	setup_xcm(&mut env);

	env.parachain_state_mut(|| {
		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.into()).into(),
			CurrencyId::Native,
			cfg(80),
			bob_in_sibling::<T>(),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.into()),
			cfg(100) - cfg(80)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.sibling_state_mut(|| {
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(xnative_curr.id(), &Keyring::Bob.into()),
			xnative_curr.val(80)
		);
	});
}

#[test_runtimes(all)]
fn transfer_foreign_tokens_to_sibling<T: Runtime + FudgeSupport>() {
	let curr = CustomCurrency {
		id: 1.into(),
		decimals: 6,
		location: Location::new(1, Parachain(T::FudgeHandle::PARA_ID)),
		custom: CustomMetadata {
			transferability: CrossChainTransferability::xcm_with_fees(0),
			..Default::default()
		},
	};

	let mut env = FudgeEnv::<T>::from_storage(
		Default::default(),
		Genesis::default()
			.add(genesis::tokens::<T>(vec![(curr.id(), curr.val(100))]))
			.add(genesis::assets::<T>(vec![&curr]))
			.storage(),
		Genesis::default()
			.add(genesis::assets::<T>(vec![&curr]))
			.storage(),
	);

	setup_xcm(&mut env);

	env.parachain_state_mut(|| {
		assert_ok!(orml_xtokens::Pallet::<T>::transfer(
			RawOrigin::Signed(Keyring::Alice.into()).into(),
			curr.id(),
			curr.val(80),
			bob_in_sibling::<T>(),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Alice.id()),
			curr.val(100) - curr.val(80)
		);
	});

	env.pass(Blocks::ByNumber(2));

	env.sibling_state_mut(|| {
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(curr.id(), &Keyring::Bob.id()),
			curr.val(80)
		);
	});
}
