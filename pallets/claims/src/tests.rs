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

use frame_support::{assert_noop, assert_ok};
use sp_core::H256;
use sp_runtime::{
	traits::{BadOrigin, Hash},
	TokenError,
};

use crate::{mock::*, *};

// ----------------------------------------------------------------------------
// Test unit cases
// ----------------------------------------------------------------------------

#[test]
fn can_upload_account() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			assert_noop!(
				Claims::can_update_upload_account(RuntimeOrigin::signed(USER_A)),
				BadOrigin
			);
			assert_ok!(Claims::can_update_upload_account(RuntimeOrigin::signed(
				ADMIN
			)));
		});
}

#[test]
fn verify_proofs() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let amount: u128 = 100 * CFG;
			let sorted_hashes_long: [H256; 31] = [
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
				[0; 32].into(),
			];

			// Abuse DDoS attach check
			assert_eq!(
				Claims::verify_proofs(&USER_B, &amount, &sorted_hashes_long.to_vec()),
				false
			);

			// Wrong sorted hashes for merkle tree
			let one_sorted_hashes: [H256; 1] = [[0; 32].into()];
			assert_eq!(
				Claims::verify_proofs(&USER_B, &amount, &one_sorted_hashes.to_vec()),
				false
			);

			let mut v: Vec<u8> = USER_B.encode();
			v.extend(amount.encode());

			// Single-leaf tree
			assert_ok!(Claims::set_upload_account(
				RuntimeOrigin::signed(ADMIN),
				ADMIN
			));
			let leaf_hash = <Runtime as frame_system::Config>::Hashing::hash(&v);
			assert_ok!(Claims::store_root_hash(
				RuntimeOrigin::signed(ADMIN),
				leaf_hash
			));
			assert_eq!(Claims::verify_proofs(&USER_B, &amount, &[].to_vec()), true);

			// Two-leaf tree
			let root_hash = Claims::sorted_hash_of(&leaf_hash, &one_sorted_hashes[0]);
			assert_ok!(Claims::store_root_hash(
				RuntimeOrigin::signed(ADMIN),
				root_hash
			));
			assert_eq!(
				Claims::verify_proofs(&USER_B, &amount, &one_sorted_hashes.to_vec()),
				true
			);

			// 10-leaf tree
			let leaf_hash_0: H256 = [0; 32].into();
			let leaf_hash_1: H256 = [1; 32].into();
			let leaf_hash_2: H256 = leaf_hash;
			let leaf_hash_3: H256 = [3; 32].into();
			let leaf_hash_4: H256 = [4; 32].into();
			let leaf_hash_5: H256 = [5; 32].into();
			let leaf_hash_6: H256 = [6; 32].into();
			let leaf_hash_7: H256 = [7; 32].into();
			let leaf_hash_8: H256 = [8; 32].into();
			let leaf_hash_9: H256 = [9; 32].into();
			let node_0 = Claims::sorted_hash_of(&leaf_hash_0, &leaf_hash_1);
			let node_1 = Claims::sorted_hash_of(&leaf_hash_2, &leaf_hash_3);
			let node_2 = Claims::sorted_hash_of(&leaf_hash_4, &leaf_hash_5);
			let node_3 = Claims::sorted_hash_of(&leaf_hash_6, &leaf_hash_7);
			let node_4 = Claims::sorted_hash_of(&leaf_hash_8, &leaf_hash_9);
			let node_00 = Claims::sorted_hash_of(&node_0, &node_1);
			let node_01 = Claims::sorted_hash_of(&node_2, &node_3);
			let node_000 = Claims::sorted_hash_of(&node_00, &node_01);
			let node_root = Claims::sorted_hash_of(&node_000, &node_4);

			let four_sorted_hashes: [H256; 4] = [
				leaf_hash_3.into(),
				node_0.into(),
				node_01.into(),
				node_4.into(),
			];
			assert_ok!(Claims::store_root_hash(
				RuntimeOrigin::signed(ADMIN),
				node_root
			));
			assert_eq!(
				Claims::verify_proofs(&USER_B, &amount, &four_sorted_hashes.to_vec()),
				true
			);
		});
}

#[test]
fn set_upload_account() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			assert_eq!(Claims::get_upload_account(), None);
			assert_noop!(
				Claims::set_upload_account(RuntimeOrigin::signed(USER_A), USER_A),
				BadOrigin
			);
			assert_ok!(Claims::set_upload_account(
				RuntimeOrigin::signed(ADMIN),
				USER_A
			));
			assert_eq!(Claims::get_upload_account(), Some(USER_A));
		});
}

#[test]
fn store_root_hash() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			assert_eq!(Claims::get_upload_account(), None);
			// USER_A not allowed to upload hash
			let root_hash = <Runtime as frame_system::Config>::Hashing::hash(&[0; 32]);
			assert_noop!(
				Claims::store_root_hash(RuntimeOrigin::signed(USER_A), root_hash),
				Error::<Runtime>::MustBeAdmin
			);
			// Adding ADMIN as allowed upload account
			assert_ok!(Claims::set_upload_account(
				RuntimeOrigin::signed(ADMIN),
				ADMIN
			));
			assert_eq!(Claims::get_upload_account(), Some(ADMIN));
			assert_ok!(Claims::store_root_hash(
				RuntimeOrigin::signed(ADMIN),
				root_hash
			));
			assert_eq!(Claims::get_root_hash(), Some(root_hash));
		});
}

fn pre_calculate_single_root(
	account_id: &<Runtime as frame_system::Config>::AccountId,
	amount: &<Runtime as pallet_balances::Config>::Balance,
	other_hash: &<Runtime as frame_system::Config>::Hash,
) -> H256 {
	let mut v: Vec<u8> = account_id.encode();
	v.extend(amount.encode());
	let leaf_hash = <Runtime as frame_system::Config>::Hashing::hash(&v);

	Claims::sorted_hash_of(&leaf_hash, other_hash)
}

#[test]
fn claim() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let amount: u128 = 100 * CFG;
			// Random sorted hashes
			let one_sorted_hashes: [H256; 1] = [[0; 32].into()];

			// proof validation error - roothash not stored
			assert_noop!(
				Claims::claim(
					RuntimeOrigin::signed(0),
					USER_B,
					amount,
					one_sorted_hashes.to_vec()
				),
				Error::<Runtime>::InvalidProofs
			);

			// Set valid proofs
			assert_ok!(Claims::set_upload_account(
				RuntimeOrigin::signed(ADMIN),
				ADMIN
			));

			let short_root_hash =
				pre_calculate_single_root(&USER_B, &(4 * CFG), &one_sorted_hashes[0]);
			assert_ok!(Claims::store_root_hash(
				RuntimeOrigin::signed(ADMIN),
				short_root_hash
			));

			// Minimum payout not met
			assert_noop!(
				Claims::claim(
					RuntimeOrigin::signed(0),
					USER_B,
					4 * CFG,
					one_sorted_hashes.to_vec()
				),
				Error::<Runtime>::UnderMinPayout
			);

			let long_root_hash =
				pre_calculate_single_root(&USER_B, &(10001 * CFG), &one_sorted_hashes[0]);
			assert_ok!(Claims::store_root_hash(
				RuntimeOrigin::signed(ADMIN),
				long_root_hash
			));

			// Claims Module Account does not have enough balance
			assert_noop!(
				Claims::claim(
					RuntimeOrigin::signed(0),
					USER_B,
					10001 * CFG,
					one_sorted_hashes.to_vec()
				),
				TokenError::FundsUnavailable
			);

			// Ok
			let ok_root_hash = pre_calculate_single_root(&USER_B, &amount, &one_sorted_hashes[0]);
			assert_ok!(Claims::store_root_hash(
				RuntimeOrigin::signed(ADMIN),
				ok_root_hash
			));

			let account_balance = <pallet_balances::Pallet<Runtime>>::free_balance(USER_B);
			assert_ok!(Claims::claim(
				RuntimeOrigin::signed(0),
				USER_B,
				amount,
				one_sorted_hashes.to_vec()
			));
			assert_eq!(Claims::get_claimed_amount(USER_B), amount);
			let account_new_balance = <pallet_balances::Pallet<Runtime>>::free_balance(USER_B);
			assert_eq!(account_new_balance, account_balance + amount);

			// Knowing that account has a balance of 100, trying to claim 50 will fail
			// Since balance logic is accumulative
			let past_root_hash =
				pre_calculate_single_root(&USER_B, &(50 * CFG), &one_sorted_hashes[0]);
			assert_ok!(Claims::store_root_hash(
				RuntimeOrigin::signed(ADMIN),
				past_root_hash
			));
			assert_noop!(
				Claims::claim(
					RuntimeOrigin::signed(0),
					USER_B,
					50 * CFG,
					one_sorted_hashes.to_vec()
				),
				Error::<Runtime>::InsufficientBalance
			);
		});
}
