// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.


//! Unit test cases for non-fungible token (NFT) processing pallet


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{
    mock::*,
    *
};

use frame_support::{
    assert_err, 
    assert_ok,
};

use sp_core::{
    H160, 
    U256,
};


// ----------------------------------------------------------------------------
// Test unit cases
// ----------------------------------------------------------------------------

#[test]
fn mint() {
    TestExternalitiesBuilder::default().build().execute_with( || {
        let asset_id = AssetId(H160::zero(), U256::zero());
        let asset_info = vec![];
        assert_ok!(NonFungibleToken::mint(&0, &1, &asset_id, asset_info));
    });
}

#[test]
fn mint_err_duplicate_id() {
    TestExternalitiesBuilder::default().build().execute_with( || {      
        let asset_id = AssetId(H160::zero(), U256::zero());
        assert_ok!(NonFungibleToken::mint(&0, &1, &asset_id, vec![]));
        assert_err!(NonFungibleToken::mint(&0, &1, &asset_id, vec![]),
                    Error::<MockRuntime>::AssetExists);
    });
}

#[test]
fn transfer() {
    TestExternalitiesBuilder::default().build().execute_with( || {
        let asset_id = AssetId(H160::zero(), U256::zero());
        // First mint to account 1
        assert_ok!(NonFungibleToken::mint(&1, &1, &asset_id, vec![]));
        // Transfer to 2
        assert_ok!(<NonFungibleToken as Unique>::transfer(&1, &2, &asset_id));
        // 2 owns asset now
        assert_eq!(<NonFungibleToken as Unique>::owner_of(&asset_id), Some(2));
    });
}

#[test]
fn transfer_err_when_not_owner() {
    TestExternalitiesBuilder::default().build().execute_with( || {
        let asset_id = AssetId(H160::zero(), U256::zero());
        // Mint to account 2
        assert_ok!(NonFungibleToken::mint(&2, &2, &asset_id, vec![]));
        // 1 transfers to 2
        assert_err!(<NonFungibleToken as Unique>::transfer(&1, &2, &asset_id),
                    Error::<MockRuntime>::NotAssetOwner);
    });
}
