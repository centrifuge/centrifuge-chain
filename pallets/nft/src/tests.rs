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

use cfg_primitives::MILLISECS_PER_DAY;
use codec::Encode;
use frame_support::{assert_err, assert_ok};
use sp_runtime::traits::{BadOrigin, Hash};

use crate::{
	mock::{helpers::*, *},
	*,
};

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

			MockFeesState::get().with(|fees| {
				assert!(fees.borrow().burn_fees.is_empty());
			});
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
				Error::<Runtime>::DocumentNotAnchored
			);

			MockFeesState::get().with(|fees| {
				assert!(fees.borrow().burn_fees.is_empty());
			});
		})
}

#[test]
fn valid_proof() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let dest_id = 0;
			let deposit_address: [u8; 20] = [0; 20];
			let pre_image = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
			let anchor_id =
				(pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
			let (proof, doc_root, static_proofs) = get_valid_proof();

			assert_ok!(Anchors::commit(
				Origin::signed(USER_B),
				pre_image,
				doc_root,
				<Runtime as frame_system::Config>::Hashing::hash_of(&0),
				MILLISECS_PER_DAY + 1
			));

			assert_ok!(ChainBridge::whitelist_chain(
				Origin::root(),
				dest_id.clone()
			));

			assert_ok!(Nft::validate_mint(
				Origin::signed(USER_A),
				anchor_id,
				deposit_address,
				vec![proof],
				static_proofs,
				0
			));

			MockFeesState::get().with(|fees| {
				assert_eq!(fees.borrow().burn_fees.len(), 1);
				assert_eq!(fees.borrow().burn_fees[0].author, USER_A);
				assert_eq!(fees.borrow().burn_fees[0].balance, NFT_PROOF_VALIDATION_FEE);
			});
		})
}

#[test]
fn invalid_proof() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let deposit_address: [u8; 20] = [0; 20];
			let pre_image = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
			let anchor_id =
				(pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
			let (proof, doc_root, static_proofs) = get_invalid_proof();

			assert_ok!(Anchors::commit(
				Origin::signed(USER_B),
				pre_image,
				doc_root,
				<Runtime as frame_system::Config>::Hashing::hash_of(&0),
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
				Error::<Runtime>::InvalidProofs
			);

			MockFeesState::get().with(|fees| {
				assert_eq!(fees.borrow().burn_fees.len(), 0);
			});
		})
}
