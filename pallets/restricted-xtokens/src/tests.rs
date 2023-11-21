// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

#![cfg(test)]

use codec::Encode;
use cumulus_primitives_core::ParaId;
use frame_support::{assert_err, assert_noop, assert_ok, traits::Currency};
use mock::*;
use orml_traits::{ConcreteFungibleAsset, MultiCurrency};
use polkadot_parachain::primitives::Sibling;
use sp_runtime::{traits::AccountIdConversion, AccountId32};
use xcm::{v3::OriginKind::SovereignAccount, VersionedXcm};
use xcm_simulator::TestExt;

use super::*;

fn para_a_account() -> AccountId32 {
	ParaId::from(1).into_account_truncating()
}

fn para_b_account() -> AccountId32 {
	ParaId::from(2).into_account_truncating()
}

fn para_d_account() -> AccountId32 {
	ParaId::from(4).into_account_truncating()
}

fn sibling_a_account() -> AccountId32 {
	Sibling::from(1).into_account_truncating()
}

fn sibling_b_account() -> AccountId32 {
	Sibling::from(2).into_account_truncating()
}

fn sibling_c_account() -> AccountId32 {
	Sibling::from(3).into_account_truncating()
}

fn sibling_d_account() -> AccountId32 {
	Sibling::from(4).into_account_truncating()
}

// Not used in any unit tests, but it's super helpful for debugging. Let's
// keep it here.
#[allow(dead_code)]
fn print_events<Runtime: frame_system::Config>(name: &'static str) {
	println!("------ {:?} events -------", name);
	frame_system::Pallet::<Runtime>::events()
		.iter()
		.for_each(|r| println!("> {:?}", r.event));
}
