// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.
//
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.
//
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use altair_runtime::{
	Balances, CurrencyIdConvert, OrmlAssetRegistry, OrmlTokens, PoolPalletIndex, RuntimeOrigin,
	XTokens,
};
use cfg_primitives::{constants::currency_decimals, parachains, Balance};
use cfg_types::{
	tokens::{CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use cfg_utils::vec_to_fixed_array;
use codec::Encode;
use frame_support::{assert_ok, traits::Len};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::{
	xcm::general_key,
	xcm_fees::{default_per_second, ksm_per_second},
};
use sp_runtime::{
	traits::{ConstU32, Convert as C2},
	WeakBoundedVec,
};
use xcm::{
	latest::{Error::BadOrigin, Junction, Junction::*, Junctions::*, MultiLocation, NetworkId},
	VersionedMultiLocation,
};
use xcm_executor::traits::Convert as C1;
use xcm_simulator::TestExt;

use crate::{
	liquidity_pools::pallet::kusama::{
		setup::{
			air, altair_account, ausd, foreign, karura_account, ksm, sibling_account, ALICE, BOB,
			KSM_ASSET_ID, PARA_ID_SIBLING,
		},
		test_net::{Altair, Karura, KusamaNet, Sibling, TestNet},
		tests::{register_air, register_ausd, register_ksm},
	},
	utils::AUSD_CURRENCY_ID,
};

#[test]
fn convert_air() {
	assert_eq!(parachains::kusama::altair::AIR_KEY.to_vec(), vec![0, 1]);

	Altair::execute_with(|| {
		// The way AIR is represented relative within the Altair runtime
		let air_location_inner: MultiLocation =
			MultiLocation::new(0, X1(general_key(parachains::kusama::altair::AIR_KEY)));

		// register air
		register_air();

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(air_location_inner),
			Ok(CurrencyId::Native),
		);

		// The canonical way AIR is represented out in the wild
		let air_location_canonical: MultiLocation = MultiLocation::new(
			1,
			X2(
				Parachain(parachains::kusama::altair::ID),
				general_key(parachains::kusama::altair::AIR_KEY),
			),
		);

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
			Some(air_location_canonical)
		)
	});
}

/// Verify that Tranche tokens are not handled by the CurrencyIdConvert
/// since we don't allow Tranche tokens to be transferable through XCM.
#[test]
fn convert_tranche() {
	let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
	let tranche_id =
		WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
	let tranche_multilocation = MultiLocation {
		parents: 1,
		interior: X3(
			Parachain(parachains::kusama::altair::ID),
			PalletInstance(PoolPalletIndex::get()),
			GeneralKey {
				length: tranche_id.len() as u8,
				data: vec_to_fixed_array(tranche_id.to_vec()),
			},
		),
	};

	Altair::execute_with(|| {
		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(tranche_multilocation),
			Err(tranche_multilocation),
		);
	});

	Altair::execute_with(|| {
		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(tranche_currency),
			None
		)
	});
}

#[test]
fn convert_ausd() {
	Altair::execute_with(|| {
		assert_eq!(parachains::kusama::karura::AUSD_KEY, &[0, 129]);

		let ausd_location: MultiLocation = MultiLocation::new(
			1,
			X2(
				Parachain(parachains::kusama::karura::ID),
				general_key(parachains::kusama::karura::AUSD_KEY),
			),
		);

		register_ausd();

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(ausd_location.clone()),
			Ok(AUSD_CURRENCY_ID),
		);

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(AUSD_CURRENCY_ID),
			Some(ausd_location)
		)
	});
}

#[test]
fn convert_ksm() {
	let ksm_location: MultiLocation = MultiLocation::parent().into();

	Altair::execute_with(|| {
		register_ksm();

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(ksm_location),
			Ok(KSM_ASSET_ID),
		);

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(KSM_ASSET_ID),
			Some(ksm_location)
		)
	});
}

#[test]
fn convert_unkown_multilocation() {
	let unknown_location: MultiLocation = MultiLocation::new(
		1,
		X2(
			Parachain(parachains::kusama::altair::ID),
			general_key(&[42]),
		),
	);

	Altair::execute_with(|| {
		assert!(<CurrencyIdConvert as C1<_, _>>::convert(unknown_location).is_err());
	});
}

#[test]
fn convert_unsupported_currency() {
	Altair::execute_with(|| {
		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Tranche(
				0,
				[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
			)),
			None
		)
	});
}
