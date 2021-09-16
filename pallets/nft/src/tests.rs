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

use crate::{
    mock::*, 
    types::AssetId, ProofVerifier, 
    *};

use codec::Encode;
use frame_support::{assert_err, assert_ok};

use runtime_common::{MILLISECS_PER_DAY, NFT_PROOF_VALIDATION_FEE, RegistryId, TokenId};
use sp_core::{H160, U256};
use sp_runtime::traits::{BadOrigin,Hash};

// ----------------------------------------------------------------------------
// Test unit cases for NFT features
// ----------------------------------------------------------------------------

#[test]
fn mint() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let asset_id = AssetId(RegistryId(H160::zero()), TokenId(U256::zero()));
			let asset_info = vec![];

            // Mint to USER_A account (by USER_DEFAULT caller)
			assert_ok!(Nft::mint(USER_DEFAULT, USER_A, asset_id, asset_info));
		});
}

#[test]
fn mint_err_duplicate_id() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let asset_id = AssetId(RegistryId(H160::zero()), TokenId(U256::zero()));

            // First mint to USER_A account (by USER_DEFAULT caller)
			assert_ok!(Nft::mint(USER_DEFAULT, USER_A, asset_id.clone(), vec![]));

            // Then try to mint to USER_A account again (still by USER_DEFAULT caller)
			assert_err!(
				Nft::mint(USER_DEFAULT, USER_A, asset_id, vec![]),
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
			
            // First mint to USER_A account
			assert_ok!(
                Nft::mint(
                    USER_A, 
                    USER_A, 
                    asset_id.clone(), 
                    vec![]
                )
            );

			// Transfer from USER_A to USER_B account (should work as USER_A owns the asset)
			assert_ok!(
				<Nft as Unique<AssetId<RegistryId, TokenId>, u64>>::transfer(
					USER_A,
					USER_B,
					asset_id.clone()
				)
			);

			// USER_B should own the asset now
			assert_eq!(
				<Nft as Unique<AssetId<RegistryId, TokenId>, u64>>::owner_of(asset_id),
				Some(USER_B)
			);
		});
}

#[test]
fn transfer_err_when_not_owner() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let asset_id = AssetId(RegistryId(H160::zero()), TokenId(U256::zero()));
			
            // USER_B mint to her/his account
			assert_ok!(Nft::mint(USER_B, USER_B, asset_id.clone(), vec![]));
			
            // Invalid transfer of the asset from USER_A to USER_B account, because USER_A does not own the asset
			assert_err!(
				<Nft as Unique<AssetId<RegistryId, TokenId>, u64>>::transfer(USER_A, USER_B, asset_id),
				Error::<MockRuntime>::NotAssetOwner
			);
		});
}

// ----------------------------------------------------------------------------
// Test unit cases for NFTs features
// ----------------------------------------------------------------------------

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
fn valid_proof() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
            let dest_id = 0;
            let deposit_address: [u8; 20] = [0; 20];
            let pre_image = <MockRuntime as frame_system::Config>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<MockRuntime as frame_system::Config>::Hashing::hash);
            let (proof, doc_root, static_proofs) = get_valid_proof();
// TODO: try only            
            let pv = ProofVerifier::<MockRuntime>::new(static_proofs);
            assert!(pv.verify_proof(doc_root, &proof));
// TODO: end of try

            assert_ok!(Anchors::commit(
                Origin::signed(USER_B),
                pre_image,
                doc_root,
                <MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
                MILLISECS_PER_DAY + 1
            ));

            assert_ok!(Chainbridge::whitelist_chain(Origin::root(), dest_id.clone()));

            assert_ok!(
                Nft::validate_mint(
                    Origin::signed(USER_A),
                    anchor_id,
                    deposit_address,
                    vec![proof],
                    static_proofs,
                    0
                )
            );

            // Account balance should be reduced (namely initial balance less validation fee)
            let account_current_balance = <pallet_balances::Pallet<MockRuntime>>::free_balance(USER_A);
            let account_expected_balance = USER_A_INITIAL_BALANCE - NFT_PROOF_VALIDATION_FEE;
            assert_eq!(account_current_balance, account_expected_balance);
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
        let (proof, doc_root, static_proofs) = get_invalid_proof();
        
        assert_ok!(Anchors::commit(
            Origin::signed(USER_B),
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
                vec![proof],
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
            Origin::signed(USER_B),
            pre_image,
            doc_root,
            <MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
            MILLISECS_PER_DAY + 1
        ));

        assert_ok!(Chainbridge::whitelist_chain(Origin::root(), dest_id.clone()));
        assert_err!(
            Nft::validate_mint(
                Origin::signed(USER_B),
                anchor_id,
                deposit_address,
                vec![pf],
                static_proofs,
                0
            ),
            // DispatchError::Module{index:0, error:3, message: Some("InsufficientBalance")}
            // TODO: Seems a better approach for error processing, rather than using module index and error codes
            //       as done with DispatchError directive. 
            pallet_balances::Error::<MockRuntime>::InsufficientBalance
        );
    })
}