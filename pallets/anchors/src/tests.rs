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

use std::time::Instant;

use codec::Encode;
use frame_support::{assert_noop, assert_ok};
use frame_system::ensure_signed;
use sp_core::H256;
use sp_runtime::traits::{BadOrigin, Hash, Header};

use super::*;
use crate::{
	common,
	mock::*,
	PRE_COMMIT_EXPIRATION_DURATION_BLOCKS, {self as pallet_anchors},
};

fn setup_blocks(blocks: u64) {
	let mut parent_hash = System::parent_hash();

	for i in 1..(blocks + 1) {
		System::initialize(&i, &parent_hash, &Default::default());

		let header = System::finalize();
		parent_hash = header.hash();
		System::set_block_number(*header.number());
	}
}

#[test]
fn basic_pre_commit() {
	new_test_ext().execute_with(|| {
		let anchor_id = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let signing_root = <Runtime as frame_system::Config>::Hashing::hash_of(&0);

		// reject unsigned
		assert_noop!(
			Anchors::pre_commit(RuntimeOrigin::none(), anchor_id, signing_root),
			BadOrigin
		);

		let origin = RuntimeOrigin::signed(1);

		assert_ok!(Anchors::pre_commit(origin.clone(), anchor_id, signing_root));

		assert_eq!(
			<Runtime as pallet_anchors::Config>::Currency::reserved_balance(
				ensure_signed(origin).unwrap()
			),
			PRE_COMMIT_FEE_VALUE,
		);

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
		let pre_image = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
		let signing_root = <Runtime as frame_system::Config>::Hashing::hash_of(&0);

		assert_eq!(
			Anchors::get_latest_anchor_index().unwrap_or_default(),
			0,
			"latest index must be 0"
		);
		// anchor
		assert_ok!(Anchors::commit(
			RuntimeOrigin::signed(1),
			pre_image,
			<Runtime as frame_system::Config>::Hashing::hash_of(&0),
			<Runtime as frame_system::Config>::Hashing::hash_of(&0),
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
			Anchors::pre_commit(RuntimeOrigin::signed(1), anchor_id, signing_root),
			Error::<Runtime>::AnchorAlreadyExists
		);
	});
}

#[test]
fn pre_commit_fail_anchor_exists_different_acc() {
	new_test_ext().execute_with(|| {
		let pre_image = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
		let signing_root = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		// anchor
		assert_ok!(Anchors::commit(
			RuntimeOrigin::signed(2),
			pre_image,
			<Runtime as frame_system::Config>::Hashing::hash_of(&0),
			<Runtime as frame_system::Config>::Hashing::hash_of(&0),
			common::MILLISECS_PER_DAY + 1
		));

		// fails because of existing anchor
		assert_noop!(
			Anchors::pre_commit(RuntimeOrigin::signed(1), anchor_id, signing_root),
			Error::<Runtime>::AnchorAlreadyExists
		);
	});
}

#[test]
fn pre_commit_fail_pre_commit_exists() {
	new_test_ext().execute_with(|| {
		let anchor_id = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let signing_root = <Runtime as frame_system::Config>::Hashing::hash_of(&0);

		// first pre-commit
		assert_ok!(Anchors::pre_commit(
			RuntimeOrigin::signed(1),
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
			Anchors::pre_commit(RuntimeOrigin::signed(1), anchor_id, signing_root),
			Error::<Runtime>::PreCommitAlreadyExists
		);

		// expire the pre-commit
		System::set_block_number(PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64 + 2);
		assert_ok!(Anchors::pre_commit(
			RuntimeOrigin::signed(1),
			anchor_id,
			signing_root
		));
	});
}

#[test]
fn pre_commit_fail_pre_commit_exists_different_acc() {
	new_test_ext().execute_with(|| {
		let anchor_id = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let signing_root = <Runtime as frame_system::Config>::Hashing::hash_of(&0);

		// first pre-commit
		assert_ok!(Anchors::pre_commit(
			RuntimeOrigin::signed(1),
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
			Anchors::pre_commit(RuntimeOrigin::signed(2), anchor_id, signing_root),
			Error::<Runtime>::PreCommitAlreadyExists
		);

		// expire the pre-commit
		System::set_block_number(PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64 + 2);
		assert_ok!(Anchors::pre_commit(
			RuntimeOrigin::signed(2),
			anchor_id,
			signing_root
		));
	});
}

#[test]
fn basic_commit() {
	new_test_ext().execute_with(|| {
		let pre_image = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
		let pre_image2 = <Runtime as frame_system::Config>::Hashing::hash_of(&1);
		let anchor_id2 =
			(pre_image2).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
		let doc_root = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		// reject unsigned
		assert_noop!(
			Anchors::commit(
				RuntimeOrigin::none(),
				pre_image,
				doc_root,
				<Runtime as frame_system::Config>::Hashing::hash_of(&0),
				1
			),
			BadOrigin
		);

		// happy
		assert_ok!(Anchors::commit(
			RuntimeOrigin::signed(1),
			pre_image,
			doc_root,
			<Runtime as frame_system::Config>::Hashing::hash_of(&0),
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
			RuntimeOrigin::signed(1),
			pre_image2,
			doc_root,
			<Runtime as frame_system::Config>::Hashing::hash_of(&0),
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
				RuntimeOrigin::signed(1),
				pre_image2,
				doc_root,
				<Runtime as frame_system::Config>::Hashing::hash_of(&0),
				2 // some arbitrary store until date that is less than the required minimum
			),
			Error::<Runtime>::AnchorStoreDateInPast
		);

		// commit anchor triggering days overflow
		assert_noop!(
			Anchors::commit(
				RuntimeOrigin::signed(1),
				pre_image2,
				doc_root,
				<Runtime as frame_system::Config>::Hashing::hash_of(&0),
				371085174374358017 // triggers overflow
			),
			Error::<Runtime>::EvictionDateTooBig
		);
	});
}

#[test]
fn commit_fail_anchor_exists() {
	new_test_ext().execute_with(|| {
		let pre_image = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
		let doc_root = <Runtime as frame_system::Config>::Hashing::hash_of(&0);

		// happy
		assert_ok!(Anchors::commit(
			RuntimeOrigin::signed(1),
			pre_image,
			doc_root,
			<Runtime as frame_system::Config>::Hashing::hash_of(&0),
			common::MILLISECS_PER_DAY + 1
		));
		// asserting that the stored anchor id is what we sent the pre-image for
		let a = Anchors::get_anchor_by_id(anchor_id).unwrap();
		assert_eq!(a.id, anchor_id);
		assert_eq!(a.doc_root, doc_root);

		assert_noop!(
			Anchors::commit(
				RuntimeOrigin::signed(1),
				pre_image,
				doc_root,
				<Runtime as frame_system::Config>::Hashing::hash_of(&0),
				common::MILLISECS_PER_DAY + 1
			),
			Error::<Runtime>::AnchorAlreadyExists
		);

		// different acc
		assert_noop!(
			Anchors::commit(
				RuntimeOrigin::signed(2),
				pre_image,
				doc_root,
				<Runtime as frame_system::Config>::Hashing::hash_of(&0),
				common::MILLISECS_PER_DAY + 1
			),
			Error::<Runtime>::AnchorAlreadyExists
		);
	});
}

#[test]
fn basic_pre_commit_commit() {
	new_test_ext().execute_with(|| {
		let pre_image = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
		let random_doc_root = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let (doc_root, signing_root, proof) = Runtime::test_document_hashes();

		// happy
		assert_ok!(Anchors::pre_commit(
			RuntimeOrigin::signed(1),
			anchor_id,
			signing_root
		));

		// wrong doc root
		assert_noop!(
			Anchors::commit(
				RuntimeOrigin::signed(1),
				pre_image,
				random_doc_root,
				proof,
				common::MILLISECS_PER_DAY + 1
			),
			Error::<Runtime>::InvalidPreCommitProof
		);

		// happy
		assert_ok!(Anchors::commit(
			RuntimeOrigin::signed(1),
			pre_image,
			doc_root,
			proof,
			common::MILLISECS_PER_DAY + 1
		));
		// asserting that the stored anchor id is what we sent the pre-image for
		let a = Anchors::get_anchor_by_id(anchor_id).unwrap();
		assert_eq!(a.id, anchor_id);
		assert_eq!(a.doc_root, doc_root);

		// Precommit is removed at succesful commit
		assert!(Anchors::get_pre_commit(anchor_id).is_none());
	});
}

#[test]
fn pre_commit_expired_when_anchoring() {
	new_test_ext().execute_with(|| {
		let pre_image = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
		let (doc_root, signing_root, proof) = Runtime::test_document_hashes();

		// happy
		assert_ok!(Anchors::pre_commit(
			RuntimeOrigin::signed(1),
			anchor_id,
			signing_root
		));
		// expire the pre-commit
		System::set_block_number(PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64 + 2);

		// happy from a different account
		assert_ok!(Anchors::commit(
			RuntimeOrigin::signed(2),
			pre_image,
			doc_root,
			proof,
			common::MILLISECS_PER_DAY + 1
		));
		// asserting that the stored anchor id is what we sent the pre-image for
		let a = Anchors::get_anchor_by_id(anchor_id).unwrap();
		assert_eq!(a.id, anchor_id);
		assert_eq!(a.doc_root, doc_root);

		// Precommit is removed at succesful commit even if it's already expired
		assert!(Anchors::get_pre_commit(anchor_id).is_none());
	});
}

#[test]
fn pre_commit_commit_fail_from_another_acc() {
	new_test_ext().execute_with(|| {
		let pre_image = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id = (pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
		let (doc_root, signing_root, proof) = Runtime::test_document_hashes();

		// happy
		assert_ok!(Anchors::pre_commit(
			RuntimeOrigin::signed(1),
			anchor_id,
			signing_root
		));

		// fail from a different account
		assert_noop!(
			Anchors::commit(
				RuntimeOrigin::signed(2),
				pre_image,
				doc_root,
				proof,
				common::MILLISECS_PER_DAY + 1
			),
			Error::<Runtime>::NotOwnerOfPreCommit
		);

		// Precommit is not removed if commit fails
		assert!(Anchors::get_pre_commit(anchor_id).is_some());
	});
}

#[test]
fn pre_commit_and_then_evict() {
	new_test_ext().execute_with(|| {
		let anchor_id_0 = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let anchor_id_1 = <Runtime as frame_system::Config>::Hashing::hash_of(&1);
		let anchor_id_2 = <Runtime as frame_system::Config>::Hashing::hash_of(&2);

		let signing_root = <Runtime as frame_system::Config>::Hashing::hash_of(&0);

		// Expiration blocks
		let block_height_0 = 0;
		let block_height_1 = 1;
		let block_height_2 = 2;

		let origin = RuntimeOrigin::signed(1);
		let account_id = ensure_signed(origin.clone()).unwrap();

		// ------ Register the pre-commits ------
		System::set_block_number(block_height_1);
		assert_ok!(Anchors::pre_commit(
			origin.clone(),
			anchor_id_0,
			signing_root
		));
		assert_ok!(Anchors::pre_commit(
			origin.clone(),
			anchor_id_1,
			signing_root
		));

		System::set_block_number(block_height_2);
		assert_ok!(Anchors::pre_commit(
			origin.clone(),
			anchor_id_2,
			signing_root
		));

		assert_eq!(
			<Runtime as pallet_anchors::Config>::Currency::reserved_balance(account_id),
			PRE_COMMIT_FEE_VALUE * 3,
		);

		// ------ Evict the pre-commits ------
		let evict_at = |block_number| {
			System::set_block_number(block_number);

			Anchors::evict_pre_commits(
				origin.clone(),
				vec![anchor_id_0, anchor_id_1, anchor_id_2]
					.try_into()
					.unwrap(),
			)
			.unwrap();
		};

		// Eviction over a non-expired anchor list.
		evict_at(block_height_0);

		assert!(Anchors::get_pre_commit(anchor_id_0).is_some());
		assert!(Anchors::get_pre_commit(anchor_id_1).is_some());
		assert!(Anchors::get_pre_commit(anchor_id_2).is_some());

		assert_eq!(
			<Runtime as pallet_anchors::Config>::Currency::reserved_balance(account_id),
			42 * 3,
		);

		// Eviction over an anchor list with some expired.
		evict_at(block_height_1 + PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64);

		assert!(Anchors::get_pre_commit(anchor_id_0).is_none());
		assert!(Anchors::get_pre_commit(anchor_id_1).is_none());
		assert!(Anchors::get_pre_commit(anchor_id_2).is_some());

		assert_eq!(
			<Runtime as pallet_anchors::Config>::Currency::reserved_balance(account_id),
			42 * 1,
		);

		// Eviction over an anchor list with some expired and others already removed.
		evict_at(block_height_2 + PRE_COMMIT_EXPIRATION_DURATION_BLOCKS as u64);

		assert!(Anchors::get_pre_commit(anchor_id_2).is_none());

		assert_eq!(
			<Runtime as pallet_anchors::Config>::Currency::reserved_balance(account_id),
			0,
		);
	});
}

#[test]
#[ignore = "sp_io::offchain::random_seed() can be called only in the offchain worker context"]
fn anchor_evict_single_anchor_per_day_many_days() {
	new_test_ext().execute_with(|| {
		let day = |n| common::MILLISECS_PER_DAY * n + 1;
		let (doc_root, _signing_root, proof) = Runtime::test_document_hashes();
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
			assert!(
				Anchors::get_evicted_anchor_root_by_day((day - 1) as u32)
					.unwrap()
					.to_vec() != [0; 32]
			);
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
		for i in 0..MAX_LOOP_IN_TX * 2 {
			let random_seed = sp_io::offchain::random_seed();
			let pre_image =
				(random_seed, i).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
			let anchor_id =
				(pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);

			assert_ok!(Anchors::commit(
				RuntimeOrigin::signed(1),
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
		<pallet_timestamp::Pallet<Runtime>>::set_timestamp(day(2));
		assert!(Anchors::get_anchor_by_id(anchors[0]).is_some());
		assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
		verify_anchor_eviction(2, &anchors);
		assert!(Anchors::get_evicted_anchor_root_by_day(2).is_some());

		verify_next_anchor_after_eviction(2, &anchors);

		const FIRST_ONES: u64 = MAX_LOOP_IN_TX / 5;
		// do the same as above for next FIRST_ONES - 1 days without child trie root
		// verification
		for i in 3..FIRST_ONES as usize + 2 {
			<pallet_timestamp::Pallet<Runtime>>::set_timestamp(day(i as u64));
			assert!(Anchors::get_anchor_by_id(anchors[i - 2]).is_some());

			// evict
			assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
			verify_anchor_eviction(i, &anchors);
			verify_next_anchor_after_eviction(i, &anchors);
		}
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			FIRST_ONES
		);

		// test out limit on the number of anchors removed at a time
		// eviction on day 2 + FIRST_ONES * MAX_LOOP_IN_TX, i.e MAX_LOOP_IN_TX + 1
		// anchors to be removed one anchor per day from the last eviction on day 2 +
		// FIRST_ONES
		<pallet_timestamp::Pallet<Runtime>>::set_timestamp(day(2 + FIRST_ONES + MAX_LOOP_IN_TX));
		assert!(
			Anchors::get_anchor_by_id(anchors[(FIRST_ONES + MAX_LOOP_IN_TX) as usize]).is_some()
		);
		// evict
		assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
		// verify anchor data has been removed until 520th anchor
		for i in (2 + FIRST_ONES) as usize..(2 + FIRST_ONES + MAX_LOOP_IN_TX) as usize {
			assert!(Anchors::get_anchor_by_id(anchors[i as usize - 2]).is_none());
			assert!(
				Anchors::get_evicted_anchor_root_by_day(i as u32)
					.unwrap()
					.to_vec() != [0; 32]
			);
		}

		assert!(
			Anchors::get_anchor_by_id(anchors[(FIRST_ONES + MAX_LOOP_IN_TX) as usize]).is_none()
		);
		assert!(
			Anchors::get_anchor_by_id(anchors[(1 + FIRST_ONES + MAX_LOOP_IN_TX) as usize])
				.is_some()
		);

		// verify that 601st anchors` indexes are left still because of 500 limit while
		// 600th anchors` indexes have been removed
		// 600th anchor
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			FIRST_ONES + MAX_LOOP_IN_TX
		);
		assert_eq!(
			Anchors::get_anchor_id_by_index(FIRST_ONES + MAX_LOOP_IN_TX).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[(FIRST_ONES - 1 + MAX_LOOP_IN_TX) as usize])
				.unwrap_or_default(),
			0
		);
		// 601st anchor indexes are left
		assert!(
			Anchors::get_anchor_id_by_index(FIRST_ONES + 1 + MAX_LOOP_IN_TX).unwrap()
				!= H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[(FIRST_ONES + MAX_LOOP_IN_TX) as usize])
				.unwrap(),
			(2 + FIRST_ONES + MAX_LOOP_IN_TX) as u32
		);

		// call evict on same day to remove the remaining indexes
		assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
		// verify that 521st anchors indexes are removed since we called a second time
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			1 + FIRST_ONES + MAX_LOOP_IN_TX
		);
		assert_eq!(
			Anchors::get_anchor_id_by_index(1 + FIRST_ONES + MAX_LOOP_IN_TX).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[(FIRST_ONES + MAX_LOOP_IN_TX) as usize])
				.unwrap_or_default(),
			0
		);

		// remove remaining anchors
		<pallet_timestamp::Pallet<Runtime>>::set_timestamp(day(MAX_LOOP_IN_TX as u64 * 2 + 1));
		assert!(Anchors::get_anchor_by_id(anchors[MAX_LOOP_IN_TX as usize * 2 - 1]).is_some());
		assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
		assert!(Anchors::get_anchor_by_id(anchors[MAX_LOOP_IN_TX as usize * 2 - 1]).is_none());
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 2
		);
		assert_eq!(
			Anchors::get_anchor_id_by_index(MAX_LOOP_IN_TX * 2).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[MAX_LOOP_IN_TX as usize * 2 - 1])
				.unwrap_or_default(),
			0
		);
	});
}

#[test]
#[ignore = "sp_io::offchain::random_seed() can be called only in the offchain worker context"]
fn test_remove_anchor_indexes() {
	new_test_ext().execute_with(|| {
		let day = |n| common::MILLISECS_PER_DAY * n + 1;
		let (doc_root, _signing_root, proof) = Runtime::test_document_hashes();

		// create MAX_LOOP_IN_TX * 4 anchors that expire on same day
		setup_blocks(100);
		for i in 0..MAX_LOOP_IN_TX * 4 {
			let random_seed = sp_io::offchain::random_seed();
			let pre_image =
				(random_seed, i).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
			let _anchor_id =
				(pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
			assert_ok!(Anchors::commit(
				RuntimeOrigin::signed(1),
				pre_image,
				doc_root,
				proof,
				// all anchors expire on same day
				day(1)
			));
		}
		assert_eq!(
			Anchors::get_latest_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 4
		);

		// first MAX_LOOP_IN_TX items
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed, MAX_LOOP_IN_TX as usize);
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX
		);

		// second MAX_LOOP_IN_TX items
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed, MAX_LOOP_IN_TX as usize);
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 2
		);

		// third MAX_LOOP_IN_TX items
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed, MAX_LOOP_IN_TX as usize);
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 3
		);

		// fourth MAX_LOOP_IN_TX items
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed, MAX_LOOP_IN_TX as usize);
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 4
		);

		// all done
		let removed = Anchors::remove_anchor_indexes(2).unwrap();
		assert_eq!(removed, 0);
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 4
		);
	});
}

#[test]
#[ignore = "sp_io::offchain::random_seed() can be called only in the offchain worker context"]
fn test_same_day_many_anchors() {
	new_test_ext().execute_with(|| {
		let day = |n| common::MILLISECS_PER_DAY * n + 1;
		let (doc_root, _signing_root, proof) = Runtime::test_document_hashes();
		let mut anchors = vec![];

		// create MAX_LOOP_IN_TX * 2 + 1 anchors that expire on same day
		setup_blocks(100);
		for i in 0..MAX_LOOP_IN_TX * 2 + 1 {
			let random_seed = sp_io::offchain::random_seed();
			let pre_image =
				(random_seed, i).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
			let anchor_id =
				(pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
			assert_ok!(Anchors::commit(
				RuntimeOrigin::signed(1),
				pre_image,
				doc_root,
				proof,
				// all anchors expire on same day
				day(1)
			));
			anchors.push(anchor_id);
		}
		assert_eq!(
			Anchors::get_latest_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 2 + 1
		);

		// first MAX_LOOP_IN_TX
		<pallet_timestamp::Pallet<Runtime>>::set_timestamp(day(2));
		assert!(Anchors::get_anchor_by_id(anchors[MAX_LOOP_IN_TX as usize * 2 - 1]).is_some());
		assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
		assert!(Anchors::get_anchor_by_id(anchors[MAX_LOOP_IN_TX as usize * 2 - 1]).is_none());
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX
		);
		assert_eq!(
			Anchors::get_anchor_id_by_index(MAX_LOOP_IN_TX).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[MAX_LOOP_IN_TX as usize - 1])
				.unwrap_or_default(),
			0
		);
		assert!(Anchors::get_evicted_anchor_root_by_day(2).is_some());

		// second MAX_LOOP_IN_TX
		assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 2
		);
		assert_eq!(
			Anchors::get_anchor_id_by_index(MAX_LOOP_IN_TX * 2).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[MAX_LOOP_IN_TX as usize * 2 - 1])
				.unwrap_or_default(),
			0
		);

		// remaining
		assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 2 + 1
		);
		assert_eq!(
			Anchors::get_anchor_id_by_index(MAX_LOOP_IN_TX * 2 + 1).unwrap_or_default(),
			H256([0; 32])
		);
		assert_eq!(
			Anchors::get_anchor_evict_date(anchors[MAX_LOOP_IN_TX as usize * 2])
				.unwrap_or_default(),
			0
		);

		// all done
		assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
		assert_eq!(
			Anchors::get_latest_evicted_anchor_index().unwrap(),
			MAX_LOOP_IN_TX * 2 + 1
		);
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
			let random_seed = sp_io::offchain::random_seed();
			let pre_image =
				(random_seed, i).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
			let anchor_id =
				(pre_image).using_encoded(<Runtime as frame_system::Config>::Hashing::hash);
			let (doc_root, signing_root, proof) = Runtime::test_document_hashes();

			// happy
			assert_ok!(Anchors::pre_commit(
				RuntimeOrigin::signed(1),
				anchor_id,
				signing_root
			));

			let now = Instant::now();

			assert_ok!(Anchors::commit(
				RuntimeOrigin::signed(1),
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

#[test]
fn evict_anchors() {
	let day = |n| common::MILLISECS_PER_DAY * n + 1;

	new_test_ext().execute_with(|| {
		assert_noop!(
			Anchors::evict_anchors(RuntimeOrigin::signed(1)),
			ArithmeticError::Underflow
		);

		<pallet_timestamp::Pallet<Runtime>>::set_timestamp(day(1));
		assert_ok!(Anchors::evict_anchors(RuntimeOrigin::signed(1)));
	});
}
