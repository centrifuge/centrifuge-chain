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

use crate::xcm::development::setup::{
	centrifuge_account, cfg, moonbeam_account, ALICE, BOB, PARA_ID_MOONBEAM,
};
use crate::xcm::development::test_net::{Development, Moonbeam, RelayChain, TestNet};
use development_runtime::{
	Balances, Connectors, CurrencyId, CustomMetadata, Origin, OrmlAssetRegistry, OrmlTokens,
	XTokens, XcmTransactor,
};
use pallet_connectors::Domain;

use frame_support::assert_noop;
use frame_support::assert_ok;
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::xcm_fees::default_per_second;
use runtime_common::{decimals, parachains, Balance, XcmMetadata};
use sp_runtime::traits::BadOrigin;
use xcm::latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId};
use xcm::prelude::{Parachain, X2};
use xcm::VersionedMultiLocation;
use xcm_emulator::TestExt;

#[test]
#[ignore] //TODO(nuno)
fn add_pool_works() {
	TestNet::reset();

	// Verify that we can successfully call Connectors::add_pool on "Centrifuge" and
	// have the targeted pool added on Moonbeam.
	// For this to work, we would need to deploy the Connectors Solidy contract on
	// Moonbeam and verify that it works but that's probably not feasible here.
	// We can start by first checking that we are able to send a transact message
	// to Moonbeam through XcmTransactor.

	Development::execute_with(|| {
		assert_ok!(Connectors::add_pool(
			Origin::signed(ALICE.into()),
			42,
			Domain::Moonbeam
		));
	});

	Moonbeam::execute_with(|| {
		assert_eq!(Balances::free_balance(&ALICE.into()), cfg(10));
	});
}
