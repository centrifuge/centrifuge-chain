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

use crate::common;
use crate::{mock::*, PRE_COMMIT_EXPIRATION_DURATION_BLOCKS};
use codec::Encode;
use frame_support::dispatch::DispatchError;
use frame_support::pallet_prelude::Hooks;
use frame_support::traits::Randomness;
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;
use sp_runtime::{
	traits::{BadOrigin, Hash, Header},
	ModuleError,
};
use std::time::Instant;

fn setup_blocks(blocks: u64) {
	let mut parent_hash = System::parent_hash();

	for i in 1..(blocks + 1) {
		System::initialize(&i, &parent_hash, &Default::default());
		RandomnessCollectiveFlip::on_initialize(i);

		let header = System::finalize();
		parent_hash = header.hash();
		System::set_block_number(*header.number());
	}
}

#[test]
fn basic_pre_commit() {
	new_test_ext().execute_with(|| {
		let anchor_id = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let signing_root = <Test as frame_system::Config>::Hashing::hash_of(&0);

		// reject unsigned
		assert_noop!(
			Anchors::pre_commit(Origin::none(), anchor_id, signing_root),
			BadOrigin
		);

		// happy
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id,
			signing_root
		));

		// asserting that the stored pre-commit has the intended values set
		let a = Anchors::get_pre_commit(anchor_id).unwrap();
		assert_eq!(a.identity, 1);
		assert_eq!(a.signing_root, signing_root);
		assert_eq!(
			a.expiration_block,
			PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64
		);
	});
}

#[test]
fn pre_commit_fail_anchor_exists() {
	new_test_ext().execute_with(|| {
		let pre_image = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
		let signing_root = <Test as frame_system::Config>::Hashing::hash_of(&0);

		assert_eq!(
			Anchors::get_latest_anchor_index().unwrap_or_default(),
			0,
			"latest index must be 0"
		);
		// anchor
		assert_ok!(Anchors::commit(
			Origin::signed(1),
			pre_image,
			<Test as frame_system::Config>::Hashing::hash_of(&0),
			<Test as frame_system::Config>::Hashing::hash_of(&0),
			common::MILLISECS_PER_DAY + 1
		));

		assert_eq!(
			Anchors::get_latest_anchor_index().unwrap_or_default(),
			1,
			"latest index must be 1"
		);
		assert_eq!(
			Anchors::get_anchor_id_by_index(1).unwrap(),
			anchor_id,
			"anchor_id must exists"
		);
		assert!(
			Anchors::get_anchor_by_id(anchor_id).is_some(),
			"anchor data must exist"
		);

		// fails because of existing anchor
		assert_noop!(
			Anchors::pre_commit(Origin::signed(1), anchor_id, signing_root),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 0,
				message: Some("AnchorAlreadyExists"),
			})
		);
	});
}

#[test]
fn pre_commit_fail_anchor_exists_different_acc() {
	new_test_ext().execute_with(|| {
		let pre_image = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
		let signing_root = <Test as frame_system::Config>::Hashing::hash_of(&0);
		// anchor
		assert_ok!(Anchors::commit(
			Origin::signed(2),
			pre_image,
			<Test as frame_system::Config>::Hashing::hash_of(&0),
			<Test as frame_system::Config>::Hashing::hash_of(&0),
			common::MILLISECS_PER_DAY + 1
		));

		// fails because of existing anchor
		assert_noop!(
			Anchors::pre_commit(Origin::signed(1), anchor_id, signing_root),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 0,
				message: Some("AnchorAlreadyExists"),
			})
		);
	});
}

#[test]
fn pre_commit_fail_pre_commit_exists() {
	new_test_ext().execute_with(|| {
		let anchor_id = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let signing_root = <Test as frame_system::Config>::Hashing::hash_of(&0);

		// first pre-commit
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id,
			signing_root
		));
		let a = Anchors::get_pre_commit(anchor_id).unwrap();
		assert_eq!(a.identity, 1);
		assert_eq!(a.signing_root, signing_root);
		assert_eq!(
			a.expiration_block,
			PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64
		);

		// fail, pre-commit exists
		assert_noop!(
			Anchors::pre_commit(Origin::signed(1), anchor_id, signing_root),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 4,
				message: Some("PreCommitAlreadyExists"),
			})
		);

		// expire the pre-commit
		System::set_block_number(PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64 + 2);
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id,
			signing_root
		));
	});
}

#[test]
fn pre_commit_fail_pre_commit_exists_different_acc() {
	new_test_ext().execute_with(|| {
		let anchor_id = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let signing_root = <Test as frame_system::Config>::Hashing::hash_of(&0);

		// first pre-commit
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id,
			signing_root
		));
		let a = Anchors::get_pre_commit(anchor_id).unwrap();
		assert_eq!(a.identity, 1);
		assert_eq!(a.signing_root, signing_root);
		assert_eq!(
			a.expiration_block,
			PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64
		);

		// fail, pre-commit exists
		assert_noop!(
			Anchors::pre_commit(Origin::signed(2), anchor_id, signing_root),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 4,
				message: Some("PreCommitAlreadyExists"),
			})
		);

		// expire the pre-commit
		System::set_block_number(PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64 + 2);
		assert_ok!(Anchors::pre_commit(
			Origin::signed(2),
			anchor_id,
			signing_root
		));
	});
}

#[test]
fn basic_commit() {
	new_test_ext().execute_with(|| {
		let pre_image = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
		let pre_image2 = <Test as frame_system::Config>::Hashing::hash_of(&1);
		let anchor_id2 = (pre_image2).using_encoded(<Test as frame_system::Config>::Hashing::hash);
		let doc_root = <Test as frame_system::Config>::Hashing::hash_of(&0);
		// reject unsigned
		assert_noop!(
			Anchors::commit(
				Origin::none(),
				pre_image,
				doc_root,
				<Test as frame_system::Config>::Hashing::hash_of(&0),
				1
			),
			BadOrigin
		);

		// happy
		assert_ok!(Anchors::commit(
			Origin::signed(1),
			pre_image,
			doc_root,
			<Test as frame_system::Config>::Hashing::hash_of(&0),
			1567589834087
		));
		// asserting that the stored anchor id is what we sent the pre-image for
		let mut a = Anchors::get_anchor_by_id(anchor_id).unwrap();
		assert_eq!(a.id, anchor_id);
		assert_eq!(a.doc_root, doc_root);
		assert_eq!(Anchors::get_anchor_evict_date(anchor_id).unwrap(), 18144);
		assert_eq!(
			Anchors::get_anchor_id_by_index(Anchors::get_latest_anchor_index().unwrap()).unwrap(),
			anchor_id
		);
		assert_eq!(Anchors::get_anchor_id_by_index(1).unwrap(), anchor_id);

		// commit second anchor to test index updates
		assert_ok!(Anchors::commit(
			Origin::signed(1),
			pre_image2,
			doc_root,
			<Test as frame_system::Config>::Hashing::hash_of(&0),
			1567589844087
		));
		a = Anchors::get_anchor_by_id(anchor_id2).unwrap();
		assert_eq!(a.id, anchor_id2);
		assert_eq!(a.doc_root, doc_root);
		assert_eq!(Anchors::get_anchor_evict_date(anchor_id2).unwrap(), 18144);
		assert_eq!(Anchors::get_anchor_id_by_index(2).unwrap(), anchor_id2);
		assert_eq!(
			Anchors::get_anchor_id_by_index(Anchors::get_latest_anchor_index().unwrap()).unwrap(),
			anchor_id2
		);

		// commit anchor with a less than required number of minimum storage days
		assert_noop!(
			Anchors::commit(
				Origin::signed(1),
				pre_image2,
				doc_root,
				<Test as frame_system::Config>::Hashing::hash_of(&0),
				2 // some arbitrary store until date that is less than the required minimum
			),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 1,
				message: Some("AnchorStoreDateInPast"),
			})
		);
	});
}

#[test]
fn commit_fail_anchor_exists() {
	new_test_ext().execute_with(|| {
		let pre_image = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
		let doc_root = <Test as frame_system::Config>::Hashing::hash_of(&0);

		// happy
		assert_ok!(Anchors::commit(
			Origin::signed(1),
			pre_image,
			doc_root,
			<Test as frame_system::Config>::Hashing::hash_of(&0),
			common::MILLISECS_PER_DAY + 1
		));
		// asserting that the stored anchor id is what we sent the pre-image for
		let a = Anchors::get_anchor_by_id(anchor_id).unwrap();
		assert_eq!(a.id, anchor_id);
		assert_eq!(a.doc_root, doc_root);

		assert_noop!(
			Anchors::commit(
				Origin::signed(1),
				pre_image,
				doc_root,
				<Test as frame_system::Config>::Hashing::hash_of(&0),
				common::MILLISECS_PER_DAY + 1
			),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 0,
				message: Some("AnchorAlreadyExists"),
			})
		);

		// different acc
		assert_noop!(
			Anchors::commit(
				Origin::signed(2),
				pre_image,
				doc_root,
				<Test as frame_system::Config>::Hashing::hash_of(&0),
				common::MILLISECS_PER_DAY + 1
			),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 0,
				message: Some("AnchorAlreadyExists"),
			})
		);
	});
}

#[test]
fn basic_pre_commit_commit() {
	new_test_ext().execute_with(|| {
		let pre_image = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
		let random_doc_root = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let (doc_root, signing_root, proof) = Test::test_document_hashes();

		// happy
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id,
			signing_root
		));

		// wrong doc root
		assert_noop!(
			Anchors::commit(
				Origin::signed(1),
				pre_image,
				random_doc_root,
				proof,
				common::MILLISECS_PER_DAY + 1
			),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 6,
				message: Some("InvalidPreCommitProof"),
			})
		);

		// happy
		assert_ok!(Anchors::commit(
			Origin::signed(1),
			pre_image,
			doc_root,
			proof,
			common::MILLISECS_PER_DAY + 1
		));
		// asserting that the stored anchor id is what we sent the pre-image for
		let a = Anchors::get_anchor_by_id(anchor_id).unwrap();
		assert_eq!(a.id, anchor_id);
		assert_eq!(a.doc_root, doc_root);
	});
}

#[test]
fn pre_commit_expired_when_anchoring() {
	new_test_ext().execute_with(|| {
		let pre_image = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
		let (doc_root, signing_root, proof) = Test::test_document_hashes();

		// happy
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id,
			signing_root
		));
		// expire the pre-commit
		System::set_block_number(PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64 + 2);

		// happy from a different account
		assert_ok!(Anchors::commit(
			Origin::signed(2),
			pre_image,
			doc_root,
			proof,
			common::MILLISECS_PER_DAY + 1
		));
		// asserting that the stored anchor id is what we sent the pre-image for
		let a = Anchors::get_anchor_by_id(anchor_id).unwrap();
		assert_eq!(a.id, anchor_id);
		assert_eq!(a.doc_root, doc_root);
	});
}

#[test]
fn pre_commit_commit_fail_from_another_acc() {
	new_test_ext().execute_with(|| {
		let pre_image = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
		let (doc_root, signing_root, proof) = Test::test_document_hashes();

		// happy
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id,
			signing_root
		));

		// fail from a different account
		assert_noop!(
			Anchors::commit(
				Origin::signed(2),
				pre_image,
				doc_root,
				proof,
				common::MILLISECS_PER_DAY + 1
			),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 5,
				message: Some("NotOwnerOfPreCommit"),
			})
		);
	});
}

// #### Pre Commit Eviction Tests
#[test]
fn pre_commit_commit_bucket_gets_determined_correctly() {
	new_test_ext().execute_with(|| {
		let current_block: <Test as frame_system::Config>::BlockNumber = 1;
		let expected_evict_bucket: <Test as frame_system::Config>::BlockNumber =
			crate::PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64
				* crate::PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER as u64;
		assert_eq!(
			Ok(expected_evict_bucket),
			Anchors::determine_pre_commit_eviction_bucket(current_block)
		);

		let current_block2: <Test as frame_system::Config>::BlockNumber = expected_evict_bucket + 1;
		let expected_evict_bucket2: <Test as frame_system::Config>::BlockNumber =
			expected_evict_bucket * 2;
		assert_eq!(
			Ok(expected_evict_bucket2),
			Anchors::determine_pre_commit_eviction_bucket(current_block2)
		);

		//testing with current bucket being even multiplier of EXPIRATION_DURATION_BLOCKS
		let current_block3: <Test as frame_system::Config>::BlockNumber = expected_evict_bucket2;
		let expected_evict_bucket3: <Test as frame_system::Config>::BlockNumber =
			expected_evict_bucket * 3;
		assert_eq!(
			Ok(expected_evict_bucket3),
			Anchors::determine_pre_commit_eviction_bucket(current_block3)
		);
	});
}

#[test]
fn put_pre_commit_into_eviction_bucket_basic_pre_commit_eviction_bucket_registration() {
	new_test_ext().execute_with(|| {
		let anchor_id_0 = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id_1 = <Test as frame_system::Config>::Hashing::hash_of(&1);
		let anchor_id_2 = <Test as frame_system::Config>::Hashing::hash_of(&2);
		let anchor_id_3 = <Test as frame_system::Config>::Hashing::hash_of(&3);

		// three different block heights that will put anchors into different eviction buckets
		let block_height_0 = 1;
		let block_height_1 =
			Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap() + block_height_0;
		let block_height_2 =
			Anchors::determine_pre_commit_eviction_bucket(block_height_1).unwrap() + block_height_0;

		// ------ First run ------
		// register anchor_id_0 into block_height_0
		System::set_block_number(block_height_0);
		assert_ok!(Anchors::put_pre_commit_into_eviction_bucket(
			anchor_id_0,
			block_height_0
		));

		let mut current_pre_commit_evict_bucket =
			Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap();

		// asserting that the right bucket was used to store
		let mut pre_commits_count =
			Anchors::get_pre_commits_count_in_evict_bucket(current_pre_commit_evict_bucket);
		assert_eq!(pre_commits_count.unwrap(), 1);
		let mut stored_pre_commit_id =
			Anchors::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 0));
		assert_eq!(stored_pre_commit_id.unwrap(), anchor_id_0);

		// ------ Second run ------
		// register anchor_id_1 and anchor_id_2 into block_height_1
		System::set_block_number(block_height_1);
		assert_ok!(Anchors::put_pre_commit_into_eviction_bucket(
			anchor_id_1,
			block_height_1
		));
		assert_ok!(Anchors::put_pre_commit_into_eviction_bucket(
			anchor_id_2,
			block_height_1
		));

		current_pre_commit_evict_bucket =
			Anchors::determine_pre_commit_eviction_bucket(block_height_1).unwrap();

		// asserting that the right bucket was used to store
		pre_commits_count =
			Anchors::get_pre_commits_count_in_evict_bucket(current_pre_commit_evict_bucket);
		assert_eq!(pre_commits_count.unwrap(), 2);
		// first pre-commit
		stored_pre_commit_id =
			Anchors::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 0));
		assert_eq!(stored_pre_commit_id.unwrap(), anchor_id_1);
		// second pre-commit
		stored_pre_commit_id =
			Anchors::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 1));
		assert_eq!(stored_pre_commit_id.unwrap(), anchor_id_2);

		// ------ Third run ------
		// register anchor_id_3 into block_height_2
		System::set_block_number(block_height_2);
		assert_ok!(Anchors::put_pre_commit_into_eviction_bucket(
			anchor_id_3,
			block_height_2
		));
		current_pre_commit_evict_bucket =
			Anchors::determine_pre_commit_eviction_bucket(block_height_2).unwrap();

		// asserting that the right bucket was used to store
		pre_commits_count =
			Anchors::get_pre_commits_count_in_evict_bucket(current_pre_commit_evict_bucket);
		assert_eq!(pre_commits_count.unwrap(), 1);
		stored_pre_commit_id =
			Anchors::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 0));
		assert_eq!(stored_pre_commit_id.unwrap(), anchor_id_3);

		// finally a sanity check that the previous bucketed items are untouched by the subsequent runs
		// checking run #1 again
		current_pre_commit_evict_bucket =
			Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap();
		pre_commits_count =
			Anchors::get_pre_commits_count_in_evict_bucket(current_pre_commit_evict_bucket);
		assert_eq!(pre_commits_count.unwrap(), 1);
		stored_pre_commit_id =
			Anchors::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 0));
		assert_eq!(stored_pre_commit_id.unwrap(), anchor_id_0);
	});
}

#[test]
fn pre_commit_with_pre_commit_eviction_bucket_registration() {
	new_test_ext().execute_with(|| {
		let anchor_id_0 = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id_1 = <Test as frame_system::Config>::Hashing::hash_of(&1);
		let anchor_id_2 = <Test as frame_system::Config>::Hashing::hash_of(&2);

		let signing_root = <Test as frame_system::Config>::Hashing::hash_of(&0);

		// three different block heights that will put anchors into different eviction buckets
		let block_height_0 = 1;
		let block_height_1 =
			Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap() + block_height_0;

		// ------ Register the pre-commits ------
		// register anchor_id_0 into block_height_0
		System::set_block_number(block_height_0);
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id_0,
			signing_root
		));

		System::set_block_number(block_height_1);
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id_1,
			signing_root
		));
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id_2,
			signing_root
		));

		// verify the pre-commits were registered
		// asserting that the stored pre-commit has the intended values set
		let pre_commit_0 = Anchors::get_pre_commit(anchor_id_0).unwrap();
		assert_eq!(pre_commit_0.identity, 1);
		assert_eq!(
			pre_commit_0.expiration_block,
			block_height_0 + PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64
		);

		// verify the registration in evict bucket of anchor 0
		let mut pre_commit_evict_bucket =
			Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap();
		let pre_commits_count =
			Anchors::get_pre_commits_count_in_evict_bucket(pre_commit_evict_bucket);
		assert_eq!(pre_commits_count.unwrap(), 1);
		let stored_pre_commit_id =
			Anchors::get_pre_commit_in_evict_bucket_by_index((pre_commit_evict_bucket, 0));
		assert_eq!(stored_pre_commit_id.unwrap(), anchor_id_0);

		// verify the expected numbers on the evict bucket IDx
		pre_commit_evict_bucket =
			Anchors::determine_pre_commit_eviction_bucket(block_height_1).unwrap();
		assert_eq!(
			Anchors::get_pre_commits_count_in_evict_bucket(pre_commit_evict_bucket).unwrap(),
			2
		);
	});
}

#[test]
fn pre_commit_and_then_evict() {
	new_test_ext().execute_with(|| {
		let anchor_id_0 = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id_1 = <Test as frame_system::Config>::Hashing::hash_of(&1);
		let anchor_id_2 = <Test as frame_system::Config>::Hashing::hash_of(&2);

		let signing_root = <Test as frame_system::Config>::Hashing::hash_of(&0);

		// three different block heights that will put anchors into different eviction buckets
		let block_height_0 = 1;
		let block_height_1 =
			Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap() + block_height_0;
		let block_height_2 =
			Anchors::determine_pre_commit_eviction_bucket(block_height_1).unwrap() + block_height_0;

		// ------ Register the pre-commits ------
		// register anchor_id_0 into block_height_0
		System::set_block_number(block_height_0);
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id_0,
			signing_root
		));

		System::set_block_number(block_height_1);
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id_1,
			signing_root
		));
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id_2,
			signing_root
		));

		// eviction fails within the "non evict time"
		System::set_block_number(
			Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap() - 1,
		);
		assert_noop!(
			Anchors::evict_pre_commits(
				Origin::signed(1),
				Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap()
			),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 10,
				message: Some("EvictionNotPossible"),
			})
		);

		// test that eviction works after expiration time
		System::set_block_number(block_height_2);
		let bucket_1 = Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap();

		// before eviction, the pre-commit data findable
		let a = Anchors::get_pre_commit(anchor_id_0).unwrap();
		assert_eq!(a.identity, 1);
		assert_eq!(a.signing_root, signing_root);

		//do check counts, evict, check counts again
		assert_eq!(
			Anchors::get_pre_commits_count_in_evict_bucket(bucket_1).unwrap(),
			1
		);
		assert_ok!(Anchors::evict_pre_commits(Origin::signed(1), bucket_1));
		assert_eq!(
			Anchors::get_pre_commits_count_in_evict_bucket(bucket_1).unwrap_or_default(),
			0
		);

		// after eviction, the pre-commit data not findable
		let a_evicted = Anchors::get_pre_commit(anchor_id_0).unwrap_or_default();
		assert_eq!(a_evicted.identity, 0);
		assert_eq!(a_evicted.expiration_block, 0);

		let bucket_2 = Anchors::determine_pre_commit_eviction_bucket(block_height_1).unwrap();
		assert_eq!(
			Anchors::get_pre_commits_count_in_evict_bucket(bucket_2).unwrap(),
			2
		);
		assert_ok!(Anchors::evict_pre_commits(Origin::signed(1), bucket_2));
		assert_eq!(
			Anchors::get_pre_commits_count_in_evict_bucket(bucket_2).unwrap_or_default(),
			0
		);
	});
}

#[test]
fn pre_commit_at_7999_and_then_evict_before_expire_and_collaborator_succeed_commit() {
	new_test_ext().execute_with(|| {
		let pre_image = <Test as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
		let (doc_root, signing_root, proof) = Test::test_document_hashes();
		// use as a start block a block that is before an eviction bucket boundary
		let start_block = PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64
			* crate::PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER as u64
			* 2 - 1;
		// expected expiry block of pre-commit
		let expiration_block = start_block + PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64; // i.e 4799 + 800

		System::set_block_number(start_block);
		// happy
		assert_ok!(Anchors::pre_commit(
			Origin::signed(1),
			anchor_id,
			signing_root
		));

		let a = Anchors::get_pre_commit(anchor_id).unwrap();
		assert_eq!(a.expiration_block, expiration_block);

		// the edge case bug we had - pre-commit eviction time is less than its expiry time
		assert_eq!(
			Anchors::determine_pre_commit_eviction_bucket(expiration_block).unwrap()
				> a.expiration_block,
			true
		);

		// this should not evict the pre-commit before its expired
		System::set_block_number(
			Anchors::determine_pre_commit_eviction_bucket(start_block).unwrap() + 1,
		);
		assert_ok!(Anchors::evict_pre_commits(
			Origin::signed(1),
			Anchors::determine_pre_commit_eviction_bucket(start_block).unwrap()
		));

		// fails
		assert_noop!(
			Anchors::commit(
				Origin::signed(2),
				pre_image,
				doc_root,
				proof,
				common::MILLISECS_PER_DAY + 1
			),
			DispatchError::Module(ModuleError {
				index: 5,
				error: 5,
				message: Some("NotOwnerOfPreCommit"),
			})
		);
	});
}

#[test]
fn pre_commit_and_then_evict_larger_than_max_evict() {
	new_test_ext().execute_with(|| {
		let block_height_0 = 1;
		let block_height_1 =
			Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap() + block_height_0;
		let signing_root = <Test as frame_system::Config>::Hashing::hash_of(&0);

		System::set_block_number(block_height_0);
		for idx in 0..crate::MAX_LOOP_IN_TX + 6 {
			assert_ok!(Anchors::pre_commit(
				Origin::signed(1),
				<Test as frame_system::Config>::Hashing::hash_of(&idx),
				signing_root
			));
		}

		System::set_block_number(block_height_1);
		let bucket_1 = Anchors::determine_pre_commit_eviction_bucket(block_height_0).unwrap();

		//do check counts, evict, check counts again
		assert_eq!(
			Anchors::get_pre_commits_count_in_evict_bucket(bucket_1).unwrap(),
			506
		);
		assert_ok!(Anchors::evict_pre_commits(Origin::signed(1), bucket_1));
		assert_eq!(
			Anchors::get_pre_commits_count_in_evict_bucket(bucket_1).unwrap(),
			6
		);

		// evict again, now should be empty
		System::set_block_number(block_height_1 + 1);
		assert_ok!(Anchors::evict_pre_commits(Origin::signed(1), bucket_1));
		assert_eq!(
			Anchors::get_pre_commits_count_in_evict_bucket(bucket_1).unwrap_or_default(),
			0
		);
	});
}
// #### End Pre Commit Eviction Tests

#[test]
fn anchor_evict_single_anchor_per_day_1000_days() {
	new_test_ext().execute_with(|| {
		let day = |n| common::MILLISECS_PER_DAY * n + 1;
		let (doc_root, _signing_root, proof) = Test::test_document_hashes();
		let mut anchors = vec![];
		let verify_anchor_eviction = |day: usize, anchors: &Vec<H256>| {
			assert!(Anchors::get_anchor_by_id(anchors[day - 2]).is_none());
			assert_eq!(
				Anchors::get_latest_evicted_anchor_index().unwrap(),
				(day - 1) as u64
			);
			assert_eq!(
				Anchors::get_anchor_id_by_index((day - 1) as u64).unwrap_or_default(),
				H256([0; 32])
			);
			assert!(Anchors::get_evicted_anchor_root_by_day((day - 1) as u32).unwrap() != [0; 32]);
			assert_eq!(
				Anchors::get_anchor_evict_date(anchors[day - 2]).unwrap_or_default(),
				0
			);
		};
		let verify_next_anchor_after_eviction = |day: usize, anchors: &Vec<H256>| {
			assert!(Anchors::get_anchor_by_id(anchors[day - 1]).is_some());
			assert_eq!(
				Anchors::get_anchor_id_by_index(day as u64).unwrap(),
				anchors[day - 1]
			);
			assert_eq!(
				Anchors::get_anchor_evict_date(anchors[day - 1]).unwrap(),
				(day + 1) as u32
			);
		};

		// create 1000 anchors one per day
		setup_blocks(100);
		for i in 0..1000 {
			let random_seed = <pallet_randomness_collective_flip::Pallet<Test>>::random_seed();
			let pre_image =
				(random_seed, i).using_encoded(<Test as frame_system::Config>::Hashing::hash);
			let anchor_id =
				(pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);

			assert_ok!(Anchors::commit(
				Origin::signed(1),
				pre_image,
				doc_root,
				proof,
				day(i + 1)
			));

			assert!(Anchors::get_anchor_by_id(anchor_id).is_some());
			assert_eq!(Anchors::get_latest_anchor_index().unwrap(), i + 1);
			assert_eq!(Anchors::get_anchor_id_by_index(i + 1).unwrap(), anchor_id);
			assert_eq!(
				Anchors::get_latest_evicted_anchor_index().unwrap_or_default(),
				0
			);
			assert_eq!(
				Anchors::get_anchor_evict_date(anchor_id).unwrap(),
				(i + 2) as u32
			);

			anchors.push(anchor_id);
		}

		// eviction on day 3
		<pallet_timestamp::Pallet<Test>>::set_timestamp(day(2));
		assert!(Anchors::get_anchor_by_id(anchors[0]).is_some());
		assert_ok!(Anchors::evict_anchors(Origin::signed(1)));
		verify_anchor_eviction(2, &anchors);
		assert_eq!(
			Anchors::get_evicted_anchor_root_by_day(2).unwrap(),
			[
				106, 177, 65, 39, 81, 119, 28, 116, 158, 148, 37, 216, 134, 138, 238, 162, 35, 32,
				214, 75, 138, 67, 134, 31, 2, 234, 148, 63, 132, 5, 213, 49
			]
		);

		verify_next_anchor_after_eviction(2, &anchors);

		// do the same as above for next 99 days without child trie root verification
		for i in 3..102 {
			<pallet_timestamp::Pallet<Test>>::set_timestamp(day(i as u64));
			assert!(Anchors::get_anchor_by_id(anchors[i - 2]).is_some());

			// evict
			assert_ok!(Anchors::evict_anchors(Origin::signed(1)));
			verify_anchor_eviction(i, &anchors);
			verify_next_anchor_after_eviction(i, &anchors);
		}
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 100);

		// test out limit on the number of anchors removed at a time
		// eviction on day 602, i.e 501 anchors to be removed one anchor
		// per day from the last eviction on day 102
		<pallet_timestamp::Pallet<Test>>::set_timestamp(day(602));
		assert!(Anchors::get_anchor_by_id(anchors[600]).is_some());
		// evict
		assert_ok!(Anchors::evict_anchors(Origin::signed(1)));
		// verify anchor data has been removed until 520th anchor
		for i in 102..602 {
			assert!(Anchors::get_anchor_by_id(anchors[i - 2]).is_none());
			assert!(Anchors::get_evicted_anchor_root_by_day(i as u32).unwrap() != [0; 32]);
		}

		assert!(Anchors::get_anchor_by_id(anchors[600]).is_none());
		assert!(Anchors::get_anchor_by_id(anchors[601]).is_some());

		// verify that 601st anchors` indexes are left still because of 500 limit while
		// 600th anchors` indexes have been removed
		// 600th anchor
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 600);
		assert_eq!(
			Anchors::get_anchor_id_by_index(600).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[599]).unwrap_or_default(),
			0
		);
		// 601st anchor indexes are left
		assert!(Anchors::get_anchor_id_by_index(601).unwrap() != H256([0; 32]));
		assert_eq!(Anchors::get_anchor_evict_date(anchors[600]).unwrap(), 602);

		// call evict on same day to remove the remaining indexes
		assert_ok!(Anchors::evict_anchors(Origin::signed(1)));
		// verify that 521st anchors indexes are removed since we called a second time
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 601);
		assert_eq!(
			Anchors::get_anchor_id_by_index(601).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[600]).unwrap_or_default(),
			0
		);

		// remove remaining anchors
		<pallet_timestamp::Pallet<Test>>::set_timestamp(day(1001));
		assert!(Anchors::get_anchor_by_id(anchors[999]).is_some());
		assert_ok!(Anchors::evict_anchors(Origin::signed(1)));
		assert!(Anchors::get_anchor_by_id(anchors[999]).is_none());
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 1000);
		assert_eq!(
			Anchors::get_anchor_id_by_index(1000).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[999]).unwrap_or_default(),
			0
		);
	});
}

#[test]
fn test_remove_anchor_indexes() {
	new_test_ext().execute_with(|| {
		let day = |n| common::MILLISECS_PER_DAY * n + 1;
		let (doc_root, _signing_root, proof) = Test::test_document_hashes();

		// create 2000 anchors that expire on same day
		setup_blocks(100);
		for i in 0..2000 {
			let random_seed = <pallet_randomness_collective_flip::Pallet<Test>>::random_seed();
			let pre_image =
				(random_seed, i).using_encoded(<Test as frame_system::Config>::Hashing::hash);
			let _anchor_id =
				(pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
			assert_ok!(Anchors::commit(
				Origin::signed(1),
				pre_image,
				doc_root,
				proof,
				// all anchors expire on same day
				day(1)
			));
		}
		assert_eq!(Anchors::get_latest_anchor_index().unwrap(), 2000);

		// first MAX_LOOP_IN_TX items
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed as u64, crate::MAX_LOOP_IN_TX);
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 500);

		// second MAX_LOOP_IN_TX items
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed as u64, crate::MAX_LOOP_IN_TX);
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 1000);

		// third MAX_LOOP_IN_TX items
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed as u64, crate::MAX_LOOP_IN_TX);
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 1500);

		// fourth MAX_LOOP_IN_TX items
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed as u64, crate::MAX_LOOP_IN_TX);
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 2000);

		// all done
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed, 0);
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 2000);
	});
}

#[test]
fn test_same_day_1001_anchors() {
	new_test_ext().execute_with(|| {
		let day = |n| common::MILLISECS_PER_DAY * n + 1;
		let (doc_root, _signing_root, proof) = Test::test_document_hashes();
		let mut anchors = vec![];

		// create 1001 anchors that expire on same day
		setup_blocks(100);
		for i in 0..1001 {
			let random_seed = <pallet_randomness_collective_flip::Pallet<Test>>::random_seed();
			let pre_image =
				(random_seed, i).using_encoded(<Test as frame_system::Config>::Hashing::hash);
			let anchor_id =
				(pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
			assert_ok!(Anchors::commit(
				Origin::signed(1),
				pre_image,
				doc_root,
				proof,
				// all anchors expire on same day
				day(1)
			));
			anchors.push(anchor_id);
		}
		assert_eq!(Anchors::get_latest_anchor_index().unwrap(), 1001);

		// first 500
		<pallet_timestamp::Pallet<Test>>::set_timestamp(day(2));
		assert!(Anchors::get_anchor_by_id(anchors[999]).is_some());
		assert_ok!(Anchors::evict_anchors(Origin::signed(1)));
		assert!(Anchors::get_anchor_by_id(anchors[999]).is_none());
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 500);
		assert_eq!(
			Anchors::get_anchor_id_by_index(500).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[499]).unwrap_or_default(),
			0
		);
		assert_eq!(
			Anchors::get_evicted_anchor_root_by_day(2).unwrap(),
			[
				46, 187, 188, 251, 253, 16, 138, 26, 49, 40, 34, 104, 1, 5, 156, 255, 11, 103, 146,
				2, 120, 3, 185, 115, 191, 116, 127, 187, 239, 227, 40, 133
			]
		);

		// second 500
		assert_ok!(Anchors::evict_anchors(Origin::signed(1)));
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 1000);
		assert_eq!(
			Anchors::get_anchor_id_by_index(1000).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[999]).unwrap_or_default(),
			0
		);

		// remaining
		assert_ok!(Anchors::evict_anchors(Origin::signed(1)));
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 1001);
		assert_eq!(
			Anchors::get_anchor_id_by_index(1001).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[1000]).unwrap_or_default(),
			0
		);

		// all done
		assert_ok!(Anchors::evict_anchors(Origin::signed(1)));
		assert_eq!(Anchors::get_latest_evicted_anchor_index().unwrap(), 1001);
	});
}

#[test]
#[ignore]
fn basic_commit_perf() {
	use std::time::{SystemTime, UNIX_EPOCH};

	new_test_ext().execute_with(|| {
		let mut elapsed: u128 = 0;
		let today_in_ms = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("Time went backwards")
			.as_millis() as u64;

		for i in 0..100000 {
			let random_seed = <pallet_randomness_collective_flip::Pallet<Test>>::random_seed();
			let pre_image =
				(random_seed, i).using_encoded(<Test as frame_system::Config>::Hashing::hash);
			let anchor_id =
				(pre_image).using_encoded(<Test as frame_system::Config>::Hashing::hash);
			let (doc_root, signing_root, proof) = Test::test_document_hashes();

			// happy
			assert_ok!(Anchors::pre_commit(
				Origin::signed(1),
				anchor_id,
				signing_root
			));

			let now = Instant::now();

			assert_ok!(Anchors::commit(
				Origin::signed(1),
				pre_image,
				doc_root,
				proof,
				today_in_ms + common::MILLISECS_PER_DAY * 2,
			));

			elapsed = elapsed + now.elapsed().as_micros();
		}

		println!("time {}", elapsed);
	});
}
