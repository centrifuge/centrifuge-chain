// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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
// Module imports and re-exports
// ----------------------------------------------------------------------------

use crate::{mock::*, *};

use codec::Encode;
use frame_support::{assert_err, assert_ok};

use crate::types::AssetId;
use runtime_common::{CFG, MILLISECS_PER_DAY, RegistryId, TokenId};
use sp_core::{H160, U256};
use sp_runtime::traits::{BadOrigin,Hash};

// ----------------------------------------------------------------------------
// Test unit cases
// ----------------------------------------------------------------------------

#[test]
fn mint() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let asset_id = AssetId(RegistryId(H160::zero()), TokenId(U256::zero()));
			let asset_info = vec![];
			assert_ok!(Nft::mint(0, 1, asset_id, asset_info));
		});
}

#[test]
fn mint_err_duplicate_id() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let asset_id = AssetId(RegistryId(H160::zero()), TokenId(U256::zero()));
			assert_ok!(Nft::mint(0, 1, asset_id.clone(), vec![]));
			assert_err!(
				Nft::mint(0, 1, asset_id, vec![]),
				Error::<MockRuntime>::AssetExists
			);
		});
}

#[test]
fn transfer() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let asset_id = AssetId(RegistryId(H160::zero()), TokenId(U256::zero()));
			// First mint to account 1
			assert_ok!(Nft::mint(1, 1, asset_id.clone(), vec![]));
			// Transfer to 2
			assert_ok!(
				<Nft as Unique<AssetId<RegistryId, TokenId>, u64>>::transfer(
					1,
					2,
					asset_id.clone()
				)
			);
			// 2 owns asset now
			assert_eq!(
				<Nft as Unique<AssetId<RegistryId, TokenId>, u64>>::owner_of(asset_id),
				Some(2)
			);
		});
}

#[test]
fn transfer_err_when_not_owner() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let asset_id = AssetId(RegistryId(H160::zero()), TokenId(U256::zero()));
			// Mint to account 2
			assert_ok!(Nft::mint(2, 2, asset_id.clone(), vec![]));
			// 1 transfers to 2
			assert_err!(
				<Nft as Unique<AssetId<RegistryId, TokenId>, u64>>::transfer(1, 2, asset_id),
				Error::<MockRuntime>::NotAssetOwner
			);
		});
}

// Unit tests imported from 'nfts' pallet
#[test]
fn bad_origin() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
        let (anchor_id, deposit_address, pfs, static_proofs, chain_id) = get_params();
        assert_err!(
            Nft::validate_mint(
                Origin::none(),
                anchor_id,
                deposit_address,
                pfs,
                static_proofs,
                chain_id
            ),
            BadOrigin
        );
    })
}

#[test]
fn missing_anchor() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
        let (anchor_id, deposit_address, pfs, static_proofs, chain_id) = get_params();
        assert_err!(
            Nft::validate_mint(
                Origin::signed(USER_A),
                anchor_id,
                deposit_address,
                pfs,
                static_proofs,
                chain_id
            ),
            Error::<MockRuntime>::DocumentNotAnchored
        );
    })
}

#[test]
fn invalid_proof() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
        let deposit_address: [u8; 20] = [0; 20];
        let pre_image = <MockRuntime as frame_system::Config>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<MockRuntime as frame_system::Config>::Hashing::hash);
        let (pf, doc_root, static_proofs) = get_invalid_proof();
        assert_ok!(Anchors::commit(
            Origin::signed(2),
            pre_image,
            doc_root,
            <MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
            MILLISECS_PER_DAY + 1
        ));

        assert_err!(
            Nft::validate_mint(
                Origin::signed(USER_A),
                anchor_id,
                deposit_address,
                vec![pf],
                static_proofs,
                0
            ),
            Error::<MockRuntime>::InvalidProofs
        );
    })
}

#[test]
fn insufficient_balance_to_mint() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
        let dest_id = 0;
        let deposit_address: [u8; 20] = [0; 20];
        let pre_image = <MockRuntime as frame_system::Config>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<MockRuntime as frame_system::Config>::Hashing::hash);
        let (pf, doc_root, static_proofs) = get_valid_proof();
        assert_ok!(Anchors::commit(
            Origin::signed(2),
            pre_image,
            doc_root,
            <MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
            MILLISECS_PER_DAY + 1
        ));

        assert_ok!(Chainbridge::whitelist_chain(Origin::root(), dest_id.clone()));
        assert_err!(
            Nft::validate_mint(
                Origin::signed(2),
                anchor_id,
                deposit_address,
                vec![pf],
                static_proofs,
                0
            ),
            pallet_balances::Error::<MockRuntime>::InsufficientBalance
            // DispatchError::Module {
            //     index: 0,
            //     error: 3,
            //     message: Some("InsufficientBalance"),
            // }
        );
    })
}

#[test]
fn valid_proof() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
        let dest_id = 0;
        let deposit_address: [u8; 20] = [0; 20];
        let pre_image = <MockRuntime as frame_system::Config>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<MockRuntime as frame_system::Config>::Hashing::hash);
        let (pf, doc_root, static_proofs) = get_valid_proof();
        assert_ok!(Anchors::commit(
            Origin::signed(2),
            pre_image,
            doc_root,
            <MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
            MILLISECS_PER_DAY + 1
        ));

        assert_ok!(Chainbridge::whitelist_chain(Origin::root(), dest_id.clone()));
        assert_ok!(Nft::validate_mint(
            Origin::signed(USER_A),
            anchor_id,
            deposit_address,
            vec![pf],
            static_proofs,
            0
        ),);

        // Account balance should be reduced amount + fee
        let account_current_balance = <pallet_balances::Pallet<MockRuntime>>::free_balance(USER_A);
        assert_eq!(account_current_balance, 90 * CFG);
    })
}
