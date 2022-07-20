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

use crate::xcm::kusama::setup::{
	air, altair_account, ausd, foreign, karura_account, ksm, sibling_account, CurrencyId, ALICE,
	BOB, PARA_ID_SIBLING,
};
use crate::xcm::kusama::test_net::{Altair, Karura, KusamaNet, Sibling, TestNet};
use altair_runtime::CurrencyIdConvert;
use altair_runtime::{Balances, CustomMetadata, Origin, OrmlAssetRegistry, OrmlTokens, XTokens};
use frame_support::assert_ok;
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::xcm_fees::{default_per_second, ksm_per_second};
use runtime_common::{decimals, parachains, Balance, XcmMetadata};
use sp_runtime::traits::Convert as C2;
use xcm::latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId};
use xcm::VersionedMultiLocation;
use xcm_emulator::TestExt;
use xcm_executor::traits::Convert as C1;
use runtime_common::xcm::general_key;

#[test]
fn convert_air() {
	assert_eq!(parachains::kusama::altair::AIR_KEY.to_vec().into(), vec![0, 1]);

	// The way AIR is represented relative within the Altair runtime
	let air_location_inner: MultiLocation = MultiLocation::new(
		0,
		X1(general_key(parachains::kusama::altair::AIR_KEY)),
	);

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

	Altair::execute_with(|| {
		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
			Some(air_location_canonical)
		)
	});
}

#[test]
fn convert_ausd() {
	assert_eq!(parachains::kusama::karura::AUSD_KEY.to_vec().into(), vec![0, 129]);

	let ausd_location: MultiLocation = MultiLocation::new(
		1,
		X2(
			Parachain(parachains::kusama::karura::ID),
			general_key(parachains::kusama::karura::AUSD_KEY),
		),
	);

	Altair::execute_with(|| {
		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(ausd_location.clone()),
			Ok(CurrencyId::AUSD),
		);

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::AUSD),
			Some(ausd_location)
		)
	});
}

#[test]
fn convert_ksm() {
	let ksm_location: MultiLocation = MultiLocation::parent().into();

	Altair::execute_with(|| {
		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(ksm_location.clone()),
			Ok(CurrencyId::KSM),
		);

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::KSM),
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
			general_key([42].to_vec()),
		),
	);

	Altair::execute_with(|| {
		assert!(<CurrencyIdConvert as C1<_, _>>::convert(unknown_location.clone()).is_err());
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
