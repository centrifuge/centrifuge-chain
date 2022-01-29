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

//! Verifiable asset (VA) registry pallet's unit test cases

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

use crate::{
	mock::{helpers::*, *},
	pallet::*,
	types::*,
};

use frame_support::{assert_err, assert_ok};

use runtime_common::{RegistryId, TokenId, MILLISECS_PER_DAY};

use sp_core::U256;

use sp_runtime::traits::Hash;

// ----------------------------------------------------------------------------
// Test unit cases
// ----------------------------------------------------------------------------

#[test]
fn mint_with_valid_proofs() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let token_id = TokenId(U256::one());
			let owner = 1;
			let origin = Origin::signed(owner);
			let (asset_id, pre_image, anchor_id, (proofs, doc_root, static_hashes), nft_data, _) =
				setup_mint::<MockRuntime>(owner, token_id);

			// Place document anchor into storage for verification
			assert_ok!(Anchors::commit(
				origin.clone(),
				pre_image,
				doc_root,
				// Proof does not matter here
				<MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
				MILLISECS_PER_DAY + 1
			));

			let (registry_id, token_id) = asset_id.destruct();

			// Mint token with document proof
			assert_ok!(Registry::mint(
				origin,
				owner,
				registry_id.clone(),
				token_id.clone(),
				nft_data.clone(),
				MintInfo {
					anchor_id: anchor_id,
					proofs: proofs,
					static_hashes: static_hashes,
				}
			));

			// Nft registered to owner
			assert_eq!(
				Nft::account_for_asset::<RegistryId, TokenId>(registry_id, token_id),
				Some(owner)
			);
		});
}

#[test]
fn mint_fails_when_dont_match_doc_root() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let token_id = TokenId(U256::one());
			let owner = 1;
			let origin = Origin::signed(owner);
			let (asset_id, pre_image, anchor_id, (proofs, _doc_root, static_hashes), nft_data, _) =
				setup_mint::<MockRuntime>(owner, token_id);

			// Place document anchor into storage for verification
			let wrong_doc_root =
				<MockRuntime as frame_system::Config>::Hashing::hash_of(&pre_image);
			assert_ok!(Anchors::commit(
				origin.clone(),
				pre_image.clone(),
				wrong_doc_root,
				// Proof does not matter here
				<MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
				MILLISECS_PER_DAY + 1
			));

			let (registry_id, token_id) = asset_id.destruct();

			// Mint token with document proof
			assert_err!(
				Registry::mint(
					origin,
					owner,
					registry_id,
					token_id,
					nft_data,
					MintInfo {
						anchor_id: anchor_id,
						proofs: proofs,
						static_hashes: static_hashes,
					}
				),
				Error::<MockRuntime>::InvalidProofs
			);
		});
}

#[test]
fn duplicate_mint_fails() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let token_id = TokenId(U256::one());
			let owner = 1;
			let origin = Origin::signed(owner);
			let (asset_id, pre_image, anchor_id, (proofs, doc_root, static_hashes), nft_data, _) =
				setup_mint::<MockRuntime>(owner, token_id);

			// Place document anchor into storage for verification
			assert_ok!(Anchors::commit(
				origin.clone(),
				pre_image,
				doc_root,
				// Proof does not matter here
				<MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
				MILLISECS_PER_DAY + 1
			));

			let (registry_id, token_id) = asset_id.destruct();

			// Mint token with document proof
			assert_ok!(Registry::mint(
				origin.clone(),
				owner,
				registry_id.clone(),
				token_id.clone(),
				nft_data.clone(),
				MintInfo {
					anchor_id: anchor_id,
					proofs: proofs.clone(),
					static_hashes: static_hashes,
				}
			));

			// Mint same token containing same id (should fail)
			assert_err!(
				Registry::mint(
					origin,
					owner,
					registry_id,
					token_id,
					nft_data.clone(),
					MintInfo {
						anchor_id: anchor_id,
						proofs: proofs,
						static_hashes: static_hashes,
					}
				),
				pallet_nft::Error::<MockRuntime>::AssetExists
			);
		});
}

#[test]
fn mint_fails_with_wrong_tokenid_in_proof() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let token_id = TokenId(U256::one());
			let owner = 1;
			let origin = Origin::signed(owner);
			let (asset_id, pre_image, anchor_id, (proofs, doc_root, static_hashes), nft_data, _) =
				setup_mint::<MockRuntime>(owner, token_id);

			// Place document anchor into storage for verification
			assert_ok!(Anchors::commit(
				origin.clone(),
				pre_image,
				doc_root,
				// Proof does not matter here
				<MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
				MILLISECS_PER_DAY + 1
			));

			let (registry_id, _) = asset_id.destruct();
			let token_id = TokenId(U256::zero());

			// Mint token with document proof
			assert_err!(
				Registry::mint(
					origin,
					owner,
					registry_id,
					token_id,
					nft_data.clone(),
					MintInfo {
						anchor_id: anchor_id,
						proofs: proofs,
						static_hashes: static_hashes,
					}
				),
				Error::<MockRuntime>::InvalidProofs
			);
		});
}

#[test]
fn create_multiple_registries() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let owner1 = 1;
			let owner2 = 1;
			let token_id = TokenId(U256::one());
			let (asset_id1, _, _, _, _, _) = setup_mint::<MockRuntime>(owner1, token_id.clone());
			let (asset_id2, _, _, _, _, _) = setup_mint::<MockRuntime>(owner2, token_id.clone());
			let (asset_id3, _, _, _, _, _) = setup_mint::<MockRuntime>(owner2, token_id);
			let (reg_id1, _) = asset_id1.destruct();
			let (reg_id2, _) = asset_id2.destruct();
			let (reg_id3, _) = asset_id3.destruct();

			assert_ne!(reg_id1, reg_id2);
			assert_ne!(reg_id1, reg_id3);
			assert_ne!(reg_id2, reg_id3);

			// Owners own their registries
			assert_eq!(Registry::get_owner(reg_id1), Some(owner1));
			assert_eq!(Registry::get_owner(reg_id2), Some(owner2));
			assert_eq!(Registry::get_owner(reg_id3), Some(owner2));
		});
}
