// Copyright 2021 Development GmbH (centrifuge.io).
// This file is part of Development chain project.
//
// Development is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Development is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// Copyright 2021 Development GmbH (centrifuge.io).
// This file is part of Development chain project.
//
// Development is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Development is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::{constants::currency_decimals, parachains, Balance};
use cfg_types::{
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use frame_support::assert_ok;
use fudge::primitives::Chain;
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use polkadot_parachain::primitives::Id;
use runtime_common::{
	xcm::general_key,
	xcm_fees::{default_per_second, ksm_per_second},
};
use sp_runtime::{traits::BadOrigin, Storage};
use tokio::runtime::Handle;
use xcm::{
	prelude::XCM_VERSION,
	v3::{Junction, Junction::*, Junctions, Junctions::*, MultiLocation, NetworkId, WeightLimit},
	VersionedMultiLocation,
};
use xcm_simulator::TestExt;

use crate::{
	chain::{
		centrifuge::{
			AccountId, Balances, OrmlAssetRegistry, OrmlTokens, PolkadotXcm, Runtime,
			RuntimeOrigin, XTokens, PARA_ID,
		},
		relay::{Hrmp as RelayHrmp, RuntimeOrigin as RelayRuntimeOrigin},
	},
	liquidity_pools::pallet::{
		development::{
			setup::{centrifuge_account, cfg, moonbeam_account},
			test_net::{Development, Moonbeam, RelayChain, TestNet},
			tests::register_ausd,
		},
		xcm_metadata,
	},
	utils::{
		accounts::Keyring,
		env,
		env::{TestEnv, PARA_ID_SIBLING},
		genesis,
	},
};

/*

NOTE: We hardcode the expected balances after an XCM operation given that the weights involved in
XCM execution often change slightly with each Polkadot update. We could simply test that the final
balance after some XCM operation is `initialBalance - amount - fee`, which would mean we would
never have to touch the tests again. However, by hard-coding these values we are forced to catch
an unexpectedly big change that would have a big impact on the weights and fees and thus balances,
which would go unnoticed and untreated otherwise.

 */

#[tokio::test]
async fn test_transfer_cfg_to_sibling() {
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_native_balances::<Runtime>(&mut genesis);
		env::test_env_with_centrifuge_storage(Handle::current(), genesis)
	};

	transfer_cfg_to_sibling(&mut env);
}

fn transfer_cfg_to_sibling(env: &mut TestEnv) {
	let alice_initial_balance = cfg(100_000);
	let transfer_amount = cfg(5);
	let cfg_in_sibling = CurrencyId::ForeignAsset(12);

	// CFG Metadata
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 18,
		name: "Development".into(),
		symbol: "CFG".into(),
		existential_deposit: 1_000_000_000_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::new(
			1,
			X2(
				Parachain(PARA_ID),
				general_key(parachains::polkadot::centrifuge::CFG_KEY),
			),
		))),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::Xcm(Default::default()),
			..CustomMetadata::default()
		},
	};

	env.with_mut_state(Chain::Para(PARA_ID), || {
		assert_eq!(
			Balances::free_balance(&Keyring::Alice.into()),
			alice_initial_balance
		);
		assert_eq!(Balances::free_balance(&moonbeam_account()), 0);

		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta.clone(),
			Some(CurrencyId::Native),
		));

		assert_ok!(PolkadotXcm::force_xcm_version(
			RuntimeOrigin::root(),
			Box::new(MultiLocation::new(
				1,
				Junctions::X1(Junction::Parachain(PARA_ID_SIBLING)),
			)),
			XCM_VERSION,
		));
	});

	env.with_mut_state(Chain::Relay, || {
		assert_ok!(RelayHrmp::force_open_hrmp_channel(
			RelayRuntimeOrigin::root(),
			Id::from(PARA_ID),
			Id::from(PARA_ID_SIBLING),
			10,
			1024,
		));

		assert_ok!(RelayHrmp::force_process_hrmp_open(
			RelayRuntimeOrigin::root(),
			0,
		));
	});

	env.evolve().unwrap();

	env.with_mut_state(Chain::Para(PARA_ID_SIBLING), || {
		assert_eq!(
			OrmlTokens::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
			0
		);

		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta,
			Some(cfg_in_sibling)
		));
	});

	env.with_mut_state(Chain::Para(PARA_ID), || {
		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(Keyring::Alice.into()),
			CurrencyId::Native,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(PARA_ID_SIBLING),
						Junction::AccountId32 {
							network: None,
							id: Keyring::Bob.into(),
						},
					),
				)
				.into()
			),
			WeightLimit::Limited(8_000_000_000_000.into()),
		));

		// Confirm that Keyring::Alice's balance is initial balance - amount transferred
		assert_eq!(
			Balances::free_balance(&Keyring::Alice.into()),
			alice_initial_balance - transfer_amount
		);

		// Verify that the amount transferred is now part of the sibling account here
		assert_eq!(Balances::free_balance(&moonbeam_account()), transfer_amount);
	});

	env.evolve().unwrap();

	env.with_mut_state(Chain::Para(PARA_ID_SIBLING), || {
		let current_balance = OrmlTokens::free_balance(cfg_in_sibling, &Keyring::Bob.into());

		// Verify that Keyring::Bob now has (amount transferred - fee)
		assert_eq!(current_balance, transfer_amount - fee(18));

		// Sanity check for the actual amount Keyring::Bob ends up with
		assert_eq!(current_balance, 4992960800000000000);
	});
}

#[tokio::test]
async fn transfer_cfg_sibling_to_centrifuge() {
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_native_balances::<Runtime>(&mut genesis);
		env::test_env_with_centrifuge_storage(Handle::current(), genesis)
	};

	// In order to be able to transfer CFG from Moonbeam to Development, we need to
	// first send CFG from Development to Moonbeam, or else it fails since it'd be
	// like Moonbeam had minted CFG on their side.
	transfer_cfg_to_sibling(&mut env);

	let para_to_sibling_transfer_amount = cfg(5);

	let alice_balance = cfg(100_000) - para_to_sibling_transfer_amount;
	let bob_balance = para_to_sibling_transfer_amount - fee(18);
	let charlie_balance = cfg(100_000);

	let sibling_to_para_transfer_amount = cfg(4);
	// Note: This asset was registered in `transfer_cfg_to_sibling`
	let cfg_in_sibling = CurrencyId::ForeignAsset(12);

	env.with_mut_state(Chain::Para(PARA_ID), || {
		assert_eq!(
			Balances::free_balance(&Keyring::Alice.into()),
			alice_balance
		);
	});

	env.with_mut_state(Chain::Para(PARA_ID_SIBLING), || {
		assert_eq!(Balances::free_balance(&centrifuge_account()), 0);

		assert_eq!(
			Balances::free_balance(&Keyring::Charlie.into()),
			charlie_balance
		);

		assert_eq!(
			OrmlTokens::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
			bob_balance
		);

		assert_ok!(PolkadotXcm::force_xcm_version(
			RuntimeOrigin::root(),
			Box::new(MultiLocation::new(
				1,
				Junctions::X1(Junction::Parachain(PARA_ID)),
			)),
			XCM_VERSION,
		));
	});

	env.with_mut_state(Chain::Relay, || {
		assert_ok!(RelayHrmp::force_open_hrmp_channel(
			RelayRuntimeOrigin::root(),
			Id::from(PARA_ID_SIBLING),
			Id::from(PARA_ID),
			10,
			1024,
		));

		assert_ok!(RelayHrmp::force_process_hrmp_open(
			RelayRuntimeOrigin::root(),
			0,
		));
	});

	env.evolve().unwrap();

	env.with_mut_state(Chain::Para(PARA_ID_SIBLING), || {
		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(Keyring::Bob.into()),
			cfg_in_sibling,
			sibling_to_para_transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(PARA_ID),
						Junction::AccountId32 {
							network: None,
							id: Keyring::Charlie.into(),
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(8_000_000_000_000.into()),
		));

		// Confirm that Charlie's balance is initial balance - amount transferred
		assert_eq!(
			OrmlTokens::free_balance(cfg_in_sibling, &Keyring::Bob.into()),
			bob_balance - sibling_to_para_transfer_amount
		);
	});

	env.evolve().unwrap();
	env.evolve().unwrap();

	env.with_mut_state(Chain::Para(PARA_ID), || {
		// Verify that Charlie's balance equals the amount transferred - fee
		assert_eq!(
			Balances::free_balance(&Into::<AccountId>::into(Keyring::Charlie)),
			charlie_balance + sibling_to_para_transfer_amount - cfg_fee(),
		);
	});
}

#[test]
fn test_total_fee() {
	assert_eq!(cfg_fee(), 8012800000000000);
}

fn cfg_fee() -> Balance {
	fee(currency_decimals::NATIVE)
}

fn ausd_fee() -> Balance {
	fee(currency_decimals::AUSD)
}

fn fee(decimals: u32) -> Balance {
	calc_fee(default_per_second(decimals))
}

// The fee associated with transferring DOT tokens
fn dot_fee() -> Balance {
	fee(10)
}

fn calc_fee(fee_per_second: Balance) -> Balance {
	// We divide the fee to align its unit and multiply by 4 as that seems to be the
	// unit of time the tests take.
	// NOTE: it is possible that in different machines this value may differ. We
	// shall see.
	fee_per_second.div_euclid(10_000) * 8
}
