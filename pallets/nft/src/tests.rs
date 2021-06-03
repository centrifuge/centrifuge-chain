// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.


//! Rad claims pallet's unit test cases


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{
    self as pallet_rad_claims,
    mock::*,
    *
};

use frame_support::{
    assert_err, assert_ok};

use sp_core::{H160, U256};

use pallet_va_registry::types::AssetId;


// ----------------------------------------------------------------------------
// Test unit cases
// ----------------------------------------------------------------------------

#[test]
fn mint() {
    TestExternalitiesBuilder::default().build().execute_with( || {
        let asset_id = AssetId(H160::zero(), U256::zero());
        let asset_info = vec![];
        assert_ok!(<SUT as Mintable>::mint(&0, &1, &asset_id, asset_info));
    });
}

#[test]
fn mint_err_duplicate_id() {
    TestExternalitiesBuilder::default().build().execute_with( || {
        let asset_id = AssetId(H160::zero(), U256::zero());
        assert_ok!(<SUT as Mintable>::mint(&0, &1, &asset_id, vec![]));
        assert_err!(<SUT as Mintable>::mint(&0, &1, &asset_id, vec![]),
                    Error::<Test>::AssetExists);
    });
}

#[test]
fn transfer() {
    TestExternalitiesBuilder::default().build().execute_with( || {
        let asset_id = AssetId(H160::zero(), U256::zero());
        // First mint to account 1
        assert_ok!(<SUT as Mintable>::mint(&1, &1, &asset_id, vec![]));
        // Transfer to 2
        assert_ok!(<SUT as Unique>::transfer(&1, &2, &asset_id));
        // 2 owns asset now
        assert_eq!(<SUT as Unique>::owner_of(&asset_id), Some(2));
    });
}

#[test]
fn transfer_err_when_not_owner() {
    TestExternalitiesBuilder::default().build().execute_with( || {
        let asset_id = AssetId(H160::zero(), U256::zero());
        // Mint to account 2
        assert_ok!(<SUT as Mintable>::mint(&2, &2, &asset_id, vec![]));
        // 1 transfers to 2
        assert_err!(<SUT as Unique>::transfer(&1, &2, &asset_id),
                    Error::<Test>::NotAssetOwner);
    });
}
