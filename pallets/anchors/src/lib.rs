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

//! # Anchors pallet for runtime
//!
//! This pallet provides functionality of Storing anchors on Chain
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	storage::child,
};
pub use pallet::*;
pub mod weights;
use scale_info::TypeInfo;
use sp_arithmetic::traits::{CheckedAdd, CheckedMul};
use sp_runtime::{traits::Hash, ArithmeticError};
use sp_std::{convert::TryInto, vec::Vec};
pub use weights::*;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod common;

/// Expiration duration in blocks of a pre-commit
/// This is the maximum expected time for document consensus to take place between a pre-commit of an anchor and a
/// commit to be received for the pre-committed anchor. Currently we expect to provide around 80 mins for this.
/// Since our current block time as per chain_spec.rs is 6s, we set this to 80 * 60 secs / 6 secs/block = 800 blocks.
const PRE_COMMIT_EXPIRATION_DURATION_BLOCKS: u32 = 800;

/// MUST be higher than 1 to assure that pre-commits are around during their validity time frame
/// The higher the number, the more pre-commits will be collected in a single eviction bucket
const PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER: u32 = 5;

/// Determines how many loop iterations are allowed to run at a time inside the runtime.
const MAX_LOOP_IN_TX: u64 = 500;

/// date 3000-01-01 -> 376200 days from unix epoch
const STORAGE_MAX_DAYS: u32 = 376200;

/// Child trie prefix
const ANCHOR_PREFIX: &[u8; 6] = b"anchor";

/// The data structure for storing pre-committed anchors.
#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PreCommitData<Hash, AccountId, BlockNumber> {
	signing_root: Hash,
	identity: AccountId,
	expiration_block: BlockNumber,
}

/// The data structure for storing committed anchors.
#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct AnchorData<Hash, BlockNumber> {
	id: Hash,
	pub doc_root: Hash,
	anchored_block: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Hash;
	use sp_std::{convert::TryInto, vec::Vec};

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_timestamp::Config + pallet_fees::Config
	{
		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
	}

	/// PreCommits store the map of anchor Id to the pre-commit, which is a lock on an anchor id to be committed later
	#[pallet::storage]
	#[pallet::getter(fn get_pre_commit)]
	pub(super) type PreCommits<T: Config> =
		StorageMap<_, Blake2_256, T::Hash, PreCommitData<T::Hash, T::AccountId, T::BlockNumber>>;

	/// Pre-commit eviction buckets maps block number and bucketID to PreCommit Hash
	#[pallet::storage]
	#[pallet::getter(fn get_pre_commit_in_evict_bucket_by_index)]
	pub(super) type PreCommitEvictionBuckets<T: Config> =
		StorageMap<_, Blake2_256, (T::BlockNumber, u64), T::Hash>;

	/// Pre-commit eviction bucket index maps block number to bucket.
	#[pallet::storage]
	#[pallet::getter(fn get_pre_commits_count_in_evict_bucket)]
	pub(super) type PreCommitEvictionBucketIndex<T: Config> =
		StorageMap<_, Blake2_256, T::BlockNumber, u64>;

	/// Map to find the eviction date given an anchor id
	#[pallet::storage]
	#[pallet::getter(fn get_anchor_evict_date)]
	pub(super) type AnchorEvictDates<T: Config> = StorageMap<_, Blake2_256, T::Hash, u32>;

	/// Map to find anchorID by index
	#[pallet::storage]
	#[pallet::getter(fn get_anchor_id_by_index)]
	pub(super) type AnchorIndexes<T: Config> = StorageMap<_, Blake2_256, u64, T::Hash>;

	/// Latest anchored index that was recently used
	#[pallet::storage]
	#[pallet::getter(fn get_latest_anchor_index)]
	pub(super) type LatestAnchorIndex<T: Config> = StorageValue<_, u64>;

	/// Latest evicted anchor index. This would keep track of the latest evicted anchor index so
	/// that we can start the removal of AnchorEvictDates index from that index onwards. Going
	/// from AnchorIndexes => AnchorEvictDates
	#[pallet::storage]
	#[pallet::getter(fn get_latest_evicted_anchor_index)]
	pub(super) type LatestEvictedAnchorIndex<T: Config> = StorageValue<_, u64>;

	/// This is to keep track of the date when a child trie of anchors was evicted last. It is
	/// to evict historic anchor data child tries if they weren't evicted in a timely manner.
	#[pallet::storage]
	#[pallet::getter(fn get_latest_evicted_date)]
	pub(super) type LatestEvictedDate<T: Config> = StorageValue<_, u32>;

	/// Storage for evicted anchor child trie roots. Anchors with a given expiry/eviction date
	/// are stored on-chain in a single child trie. This child trie is removed after the expiry
	/// date has passed while its root is stored permanently for proving an existence of an
	/// evicted anchor.
	#[pallet::storage]
	#[pallet::getter(fn get_evicted_anchor_root_by_day)]
	pub(super) type EvictedAnchorRoots<T: Config> = StorageMap<_, Blake2_256, u32, Vec<u8>>;

	#[pallet::error]
	pub enum Error<T> {
		/// Anchor with anchor_id already exists
		AnchorAlreadyExists,

		/// Anchor store date must be in now or future
		AnchorStoreDateInPast,

		/// Anchor store date must not be more than max store date
		AnchorStoreDateAboveMaxLimit,

		/// State rent fee not set in the Fee Pallet
		FeeNotSet,

		/// Pre-commit already exists
		PreCommitAlreadyExists,

		/// Sender is not the owner of pre commit
		NotOwnerOfPreCommit,

		/// Invalid pre commit proof
		InvalidPreCommitProof,

		/// Pre Commit expiration block too big
		PreCommitExpirationTooBig,

		/// Eviction date too big for conversion
		EvictionDateTooBig,

		/// Failed to convert epoch in MS to days
		FailedToConvertEpochToDays,

		/// Bucket eviction not possible
		EvictionNotPossible,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Obtains an exclusive lock to make the next update to a certain document version
		/// identified by `anchor_id` on Centrifuge p2p network for a number of blocks given
		/// by `pre_commit_expiration_duration_blocks` function. `signing_root` is a child node of
		/// the off-chain merkle tree of that document. In Centrifuge protocol, A document is
		/// committed only after reaching consensus with the other collaborators on the document.
		/// Consensus is reached by getting a cryptographic signature from other parties by
		/// sending them the `signing_root`. To deny the counter-party the free option of publishing
		/// its own state commitment upon receiving a request for signature, the node can first
		/// publish a pre-commit. Only the pre-committer account in the Centrifuge chain is
		/// allowed to `commit` a corresponding anchor before the pre-commit has expired.
		/// For a more detailed explanation refer section 3.4 of
		/// [Centrifuge Protocol Paper](https://staticw.centrifuge.io/assets/centrifuge_os_protocol_paper.pdf)
		#[pallet::weight(<T as pallet::Config>::WeightInfo::pre_commit())]
		pub fn pre_commit(
			origin: OriginFor<T>,
			anchor_id: T::Hash,
			signing_root: T::Hash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				Self::get_anchor_by_id(anchor_id).is_none(),
				Error::<T>::AnchorAlreadyExists
			);
			ensure!(
				!Self::has_valid_pre_commit(anchor_id),
				Error::<T>::PreCommitAlreadyExists
			);

			let expiration_block = <frame_system::Pallet<T>>::block_number()
				.checked_add(&T::BlockNumber::from(PRE_COMMIT_EXPIRATION_DURATION_BLOCKS))
				.ok_or(ArithmeticError::Overflow)?;
			<PreCommits<T>>::insert(
				anchor_id,
				PreCommitData {
					signing_root,
					identity: who.clone(),
					expiration_block,
				},
			);

			Self::put_pre_commit_into_eviction_bucket(anchor_id, expiration_block)?;
			Ok(())
		}

		/// Commits a `document_root` of a merklized off chain document in Centrifuge p2p network as
		/// the latest version id(`anchor_id`) obtained by hashing `anchor_id_preimage`. If a
		/// pre-commit exists for the obtained `anchor_id`, hash of pre-committed
		/// `signing_root + proof` must match the given `doc_root`. To avoid state bloat on chain,
		/// the committed anchor would be evicted after the given `stored_until_date`. The calling
		/// account would be charged accordingly for the storage period.
		/// For a more detailed explanation refer section 3.4 of
		/// [Centrifuge Protocol Paper](https://staticw.centrifuge.io/assets/centrifuge_os_protocol_paper.pdf)
		#[pallet::weight(<T as pallet::Config>::WeightInfo::commit())]
		pub fn commit(
			origin: OriginFor<T>,
			anchor_id_preimage: T::Hash,
			doc_root: T::Hash,
			proof: T::Hash,
			stored_until_date: T::Moment,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// validate the eviction date
			let eviction_date_u64 = TryInto::<u64>::try_into(stored_until_date)
				.or(Err(Error::<T>::EvictionDateTooBig))?;
			let nowt = <pallet_timestamp::Pallet<T>>::get();
			let now: u64 =
				TryInto::<u64>::try_into(nowt).or(Err(Error::<T>::EvictionDateTooBig))?;

			ensure!(
				now + common::MILLISECS_PER_DAY < eviction_date_u64,
				Error::<T>::AnchorStoreDateInPast
			);

			let stored_until_date_from_epoch = common::get_days_since_epoch(eviction_date_u64);
			ensure!(
				stored_until_date_from_epoch <= STORAGE_MAX_DAYS,
				Error::<T>::AnchorStoreDateAboveMaxLimit
			);

			let anchor_id =
				(anchor_id_preimage).using_encoded(<T as frame_system::Config>::Hashing::hash);
			ensure!(
				Self::get_anchor_by_id(anchor_id).is_none(),
				Error::<T>::AnchorAlreadyExists
			);

			if Self::has_valid_pre_commit(anchor_id) {
				<PreCommits<T>>::get(anchor_id)
					.filter(|pre_commit| pre_commit.identity == who.clone())
					.ok_or(Error::<T>::NotOwnerOfPreCommit)?;
				ensure!(
					Self::has_valid_pre_commit_proof(anchor_id, doc_root, proof),
					Error::<T>::InvalidPreCommitProof
				);
			}

			// pay the state rent
			let today_in_days_from_epoch =
				TryInto::<u64>::try_into(<pallet_timestamp::Pallet<T>>::get())
					.map(common::get_days_since_epoch)
					.or(Err(Error::<T>::FailedToConvertEpochToDays))?;

			// TODO(dev): move the fee to treasury account once its integrated instead of burning fee
			// we use the fee config setup on genesis for anchoring to calculate the state rent
			let base_fee =
				<pallet_fees::Pallet<T>>::price_of(Self::fee_key()).ok_or(Error::<T>::FeeNotSet)?;
			let multiplier = stored_until_date_from_epoch
				.checked_sub(today_in_days_from_epoch)
				.ok_or(ArithmeticError::Underflow)?;

			let fee = base_fee
				.checked_mul(&pallet_fees::BalanceOf::<T>::from(multiplier))
				.ok_or(ArithmeticError::Overflow)?;

			// pay state rent to block author
			<pallet_fees::Pallet<T>>::pay_fee_to_author(who, fee)?;

			let anchored_block = <frame_system::Pallet<T>>::block_number();
			let anchor_data = AnchorData {
				id: anchor_id,
				doc_root,
				anchored_block,
			};

			let prefixed_key = Self::anchor_storage_key(&stored_until_date_from_epoch.encode());
			Self::store_anchor(
				anchor_id,
				&prefixed_key,
				stored_until_date_from_epoch,
				&anchor_data.encode(),
			)?;
			Ok(())
		}

		/// Initiates eviction of pre-commits that has expired given that the current block number
		/// has progressed past the block number provided in `evict_bucket`. `evict_bucket` is also
		/// the index to find the pre-commits stored in storage to be evicted when the
		/// `evict_bucket` number of blocks has expired.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::evict_pre_commits())]
		pub fn evict_pre_commits(
			origin: OriginFor<T>,
			evict_bucket: T::BlockNumber,
		) -> DispatchResult {
			ensure_signed(origin)?;
			ensure!(
				<frame_system::Pallet<T>>::block_number() >= evict_bucket,
				Error::<T>::EvictionNotPossible
			);

			let pre_commits_count =
				Self::get_pre_commits_count_in_evict_bucket(evict_bucket).unwrap_or_default();
			for idx in (0..pre_commits_count).rev() {
				if pre_commits_count - idx > MAX_LOOP_IN_TX {
					break;
				}

				Self::get_pre_commit_in_evict_bucket_by_index((evict_bucket, idx))
					.map(|pre_commit_id| <PreCommits<T>>::remove(pre_commit_id));

				<PreCommitEvictionBuckets<T>>::remove((evict_bucket, idx));

				// decreases the evict bucket item count or remove index completely if empty
				if idx == 0 {
					<PreCommitEvictionBucketIndex<T>>::remove(evict_bucket);
				} else {
					<PreCommitEvictionBucketIndex<T>>::insert(evict_bucket, idx);
				}
			}
			Ok(())
		}

		/// Initiates eviction of expired anchors. Since anchors are stored on a child trie indexed by
		/// their eviction date, what this function does is to remove those child tries which has
		/// date_represented_by_root < current_date. Additionally it needs to take care of indexes
		/// created for accessing anchors, eg: to find an anchor given an id.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::evict_anchors())]
		pub fn evict_anchors(origin: OriginFor<T>) -> DispatchResult {
			ensure_signed(origin)?;
			let current_timestamp = <pallet_timestamp::Pallet<T>>::get();

			// get the today counting epoch, so that we can remove the corresponding child trie
			let today_in_days_from_epoch = TryInto::<u64>::try_into(current_timestamp)
				.map(common::get_days_since_epoch)
				.or(Err(Error::<T>::FailedToConvertEpochToDays))?;
			let evict_date = <LatestEvictedDate<T>>::get()
				.unwrap_or_default()
				.checked_add(1)
				.ok_or(ArithmeticError::Overflow)?;

			// store yesterday as the last day of eviction
			let yesterday = today_in_days_from_epoch
				.checked_sub(1)
				.ok_or(ArithmeticError::Underflow)?;

			// remove child tries starting from day next to last evicted day
			let _evicted_trie_count =
				Self::evict_anchor_child_tries(evict_date, today_in_days_from_epoch);
			let _evicted_anchor_indexes_count = Self::remove_anchor_indexes(yesterday)?;
			<LatestEvictedDate<T>>::put(yesterday);

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Checks if the given `anchor_id` has a valid pre-commit, i.e it has a pre-commit with
	/// `expiration_block` < `current_block_number`.
	fn has_valid_pre_commit(anchor_id: T::Hash) -> bool {
		match <PreCommits<T>>::get(anchor_id) {
			Some(pre_commit) => {
				pre_commit.expiration_block > <frame_system::Pallet<T>>::block_number()
			}
			None => false,
		}
	}

	/// Checks if `hash(signing_root, proof) == doc_root` for the given `anchor_id`. Concatenation
	/// of `signing_root` and `proof` is done in a fixed order (signing_root + proof).
	/// assumes there is a valid pre-commit under PreCommits
	fn has_valid_pre_commit_proof(anchor_id: T::Hash, doc_root: T::Hash, proof: T::Hash) -> bool {
		let signing_root = match <PreCommits<T>>::get(anchor_id) {
			Some(pre_commit) => pre_commit.signing_root,
			None => return false,
		};
		let mut concatenated_bytes = signing_root.as_ref().to_vec();
		let proof_bytes = proof.as_ref().to_vec();

		// concat hashes
		concatenated_bytes.extend(proof_bytes);
		let calculated_root = <T as frame_system::Config>::Hashing::hash(&concatenated_bytes);
		return doc_root == calculated_root;
	}

	/// Puts the pre-commit (based on anchor_id) into the correct eviction bucket
	fn put_pre_commit_into_eviction_bucket(
		anchor_id: T::Hash,
		expiration_block: T::BlockNumber,
	) -> DispatchResult {
		// determine which eviction bucket to put into
		let evict_after_block = Self::determine_pre_commit_eviction_bucket(expiration_block)?;

		// get current index in eviction bucket and increment
		let eviction_bucket_size =
			Self::get_pre_commits_count_in_evict_bucket(evict_after_block).unwrap_or_default();

		let idx = eviction_bucket_size
			.checked_add(1)
			.ok_or(ArithmeticError::Overflow)?;

		// add to eviction bucket and update bucket counter
		<PreCommitEvictionBuckets<T>>::insert(
			(evict_after_block.clone(), eviction_bucket_size.clone()),
			anchor_id,
		);
		<PreCommitEvictionBucketIndex<T>>::insert(evict_after_block, idx);
		Ok(())
	}

	/// Determines the next eviction bucket number based on the given BlockNumber
	/// This can be used to determine which eviction bucket a pre-commit
	/// should be put into for later eviction.
	fn determine_pre_commit_eviction_bucket(
		pre_commit_expiration_block: T::BlockNumber,
	) -> Result<T::BlockNumber, DispatchError> {
		let expiration_horizon = PRE_COMMIT_EXPIRATION_DURATION_BLOCKS
			.checked_mul(PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER)
			.ok_or(ArithmeticError::Overflow)?;
		let expiration_block = TryInto::<u32>::try_into(pre_commit_expiration_block)
			.or(Err(Error::<T>::PreCommitExpirationTooBig))?;
		expiration_block
			.checked_sub(expiration_block % expiration_horizon)
			.ok_or(ArithmeticError::Underflow)
			.and_then(|expiration_block| {
				expiration_block
					.checked_add(expiration_horizon)
					.ok_or(ArithmeticError::Overflow)
			})
			.and_then(|put_into_bucket| Ok(T::BlockNumber::from(put_into_bucket)))
			.or_else(|res| Err(DispatchError::Arithmetic(res)))
	}

	/// Remove child tries starting with `from` day to `until` day returning the
	/// count of tries removed.
	fn evict_anchor_child_tries(from: u32, until: u32) -> usize {
		(from..until)
			.map(|day| {
				(
					day,
					common::generate_child_storage_key(&Self::anchor_storage_key(&day.encode())),
				)
			})
			// store the root of child trie for the day on chain before eviction. Checks if it
			// exists before hand to ensure that it doesn't overwrite a root.
			.map(|(day, key)| {
				if !<EvictedAnchorRoots<T>>::contains_key(day) {
					<EvictedAnchorRoots<T>>::insert(day, child::root(&key));
				}
				key
			})
			.map(|key| child::kill_storage(&key, None))
			.count()
	}

	/// Iterate from the last evicted anchor to latest anchor, while removing indexes that
	/// are no longer valid because they belong to an expired/evicted anchor. The loop is
	/// only allowed to run MAX_LOOP_IN_TX at a time.
	fn remove_anchor_indexes(yesterday: u32) -> Result<usize, DispatchError> {
		let evicted_index = <LatestEvictedAnchorIndex<T>>::get()
			.unwrap_or_default()
			.checked_add(1)
			.ok_or(ArithmeticError::Overflow)?;
		let anchor_index = <LatestAnchorIndex<T>>::get()
			.unwrap_or_default()
			.checked_add(1)
			.ok_or(ArithmeticError::Overflow)?;
		let count = (evicted_index..anchor_index)
			// limit to only MAX_LOOP_IN_TX number of anchor indexes to remove
			.take(MAX_LOOP_IN_TX as usize)
			// get eviction date of the anchor given by index
			.filter_map(|idx| {
				let anchor_index = <AnchorIndexes<T>>::get(idx)?;
				let eviction_date = <AnchorEvictDates<T>>::get(anchor_index).unwrap_or_default();
				Some((idx, anchor_index, eviction_date))
			})
			// filter out evictable anchors, anchor_evict_date can be 0 when evicting before any anchors are created
			.filter(|(_, _, anchor_evict_date)| anchor_evict_date <= &yesterday)
			// remove indexes
			.map(|(idx, anchor_index, _)| {
				<AnchorEvictDates<T>>::remove(anchor_index);
				<AnchorIndexes<T>>::remove(idx);
				<LatestEvictedAnchorIndex<T>>::put(idx);
			})
			.count();
		Ok(count)
	}

	/// Get an anchor by its id in the child storage
	pub fn get_anchor_by_id(anchor_id: T::Hash) -> Option<AnchorData<T::Hash, T::BlockNumber>> {
		let anchor_evict_date = <AnchorEvictDates<T>>::get(anchor_id)?;
		let anchor_evict_date_enc: &[u8] = &anchor_evict_date.encode();
		let prefixed_key = Self::anchor_storage_key(anchor_evict_date_enc);
		let child_info = common::generate_child_storage_key(&prefixed_key);

		child::get_raw(&child_info, anchor_id.as_ref())
			.and_then(|data| AnchorData::decode(&mut &*data).ok())
	}

	pub fn anchor_storage_key(storage_key: &[u8]) -> Vec<u8> {
		let mut prefixed_key = Vec::with_capacity(ANCHOR_PREFIX.len() + storage_key.len());
		prefixed_key.extend_from_slice(ANCHOR_PREFIX);
		prefixed_key.extend_from_slice(storage_key);
		prefixed_key
	}

	fn store_anchor(
		anchor_id: T::Hash,
		prefixed_key: &Vec<u8>,
		stored_until_date_from_epoch: u32,
		anchor_data_encoded: &[u8],
	) -> DispatchResult {
		let idx = <LatestAnchorIndex<T>>::get()
			.unwrap_or_default()
			.checked_add(1)
			.ok_or(ArithmeticError::Overflow)?;

		let child_info = common::generate_child_storage_key(prefixed_key);
		child::put_raw(&child_info, anchor_id.as_ref(), &anchor_data_encoded);

		// update indexes
		<AnchorEvictDates<T>>::insert(&anchor_id, &stored_until_date_from_epoch);
		<AnchorIndexes<T>>::insert(idx, &anchor_id);
		<LatestAnchorIndex<T>>::put(idx);
		Ok(())
	}

	fn fee_key() -> <T as frame_system::Config>::Hash {
		<T as frame_system::Config>::Hashing::hash_of(&0)
	}
}
