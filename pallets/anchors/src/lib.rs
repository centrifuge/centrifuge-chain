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
//
//! # Anchors Pallet
//!
//! This pallet provides functionality of Storing anchors on Chain.

#![cfg_attr(not(feature = "std"), no_std)]
// This pallet is getting a big refactor soon, so no sense doing clippy cleanups
#![allow(clippy::all)]

use frame_support::{
	pallet_prelude::RuntimeDebug,
	traits::{Currency, Get},
};
pub use pallet::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::marker::PhantomData;
pub use weights::*;
pub mod weights;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod common;

/// Type to get the length of hash
pub struct RootHashSize<T>(PhantomData<T>);
impl<T: Config> Get<u32> for RootHashSize<T> {
	fn get() -> u32 {
		<T::Hashing as sp_core::Hasher>::LENGTH as u32
	}
}

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Expiration duration in blocks of a pre-commit
/// This is the maximum expected time for document consensus to take place
/// between a pre-commit of an anchor and a commit to be received for the
/// pre-committed anchor. Currently we expect to provide around 80 mins for
/// this. Since our current block time as per chain_spec.rs is 6s, we set this
/// to 80 * 60 secs / 6 secs/block = 800 blocks.
pub const PRE_COMMIT_EXPIRATION_DURATION_BLOCKS: u32 = 800;

/// Determines how many loop iterations are allowed to run at a time inside the
/// runtime.
const MAX_LOOP_IN_TX: u64 = 100;

/// date 3000-01-01 -> 376200 days from unix epoch
const STORAGE_MAX_DAYS: u32 = 376200;

/// Child trie prefix
const ANCHOR_PREFIX: &[u8; 6] = b"anchor";

/// Determines the max size of the input list used in the precommit eviction.
const EVICT_PRE_COMMIT_LIST_SIZE: u32 = 100;

/// The data structure for storing pre-committed anchors.
#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PreCommitData<Hash, AccountId, BlockNumber, Balance> {
	signing_root: Hash,
	identity: AccountId,
	expiration_block: BlockNumber,
	deposit: Balance,
}

/// The data structure for storing committed anchors.
#[derive(Encode, Decode, Default, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct AnchorData<Hash, BlockNumber> {
	id: Hash,
	pub doc_root: Hash,
	anchored_block: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use cfg_traits::fees::{Fee, Fees};
	use frame_support::{pallet_prelude::*, storage::child, traits::ReservableCurrency};
	use frame_system::pallet_prelude::*;
	use sp_runtime::{
		traits::{CheckedAdd, CheckedMul, Hash},
		ArithmeticError, StateVersion,
	};
	use sp_std::vec::Vec;

	use super::*;

	// Simple declaration of the `Pallet` type. It is placeholder we use to
	// implement traits and method.
	#[pallet::pallet]

	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_timestamp::Config {
		/// Entity used to pay fees
		type Fees: Fees<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;

		/// Key used to retrieve the fee balances in the commit method.
		type CommitAnchorFeeKey: Get<<Self::Fees as Fees>::FeeKey>;

		/// Key to identify the amount of funds reserved in a
		/// [`Pallet::pre_commit()`] call. These funds will be unreserved once
		/// the user make the [`Pallet::commit()`] succesfully
		/// or call [`Pallet::evict_pre_commits()`]
		type PreCommitDepositFeeKey: Get<<Self::Fees as Fees>::FeeKey>;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;

		/// Currency as viewed from this pallet
		type Currency: ReservableCurrency<Self::AccountId>;
	}

	/// PreCommits store the map of anchor Id to the pre-commit, which is a lock
	/// on an anchor id to be committed later
	#[pallet::storage]
	#[pallet::getter(fn get_pre_commit)]
	pub(super) type PreCommits<T: Config> = StorageMap<
		_,
		Blake2_256,
		T::Hash,
		PreCommitData<T::Hash, T::AccountId, BlockNumberFor<T>, BalanceOf<T>>,
	>;

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

	/// Latest evicted anchor index. This would keep track of the latest evicted
	/// anchor index so that we can start the removal of AnchorEvictDates index
	/// from that index onwards. Going from AnchorIndexes => AnchorEvictDates
	#[pallet::storage]
	#[pallet::getter(fn get_latest_evicted_anchor_index)]
	pub(super) type LatestEvictedAnchorIndex<T: Config> = StorageValue<_, u64>;

	/// This is to keep track of the date when a child trie of anchors was
	/// evicted last. It is to evict historic anchor data child tries if they
	/// weren't evicted in a timely manner.
	#[pallet::storage]
	#[pallet::getter(fn get_latest_evicted_date)]
	pub(super) type LatestEvictedDate<T: Config> = StorageValue<_, u32>;

	/// Storage for evicted anchor child trie roots. Anchors with a given
	/// expiry/eviction date are stored on-chain in a single child trie. This
	/// child trie is removed after the expiry date has passed while its root is
	/// stored permanently for proving an existence of an evicted anchor.
	#[pallet::storage]
	#[pallet::getter(fn get_evicted_anchor_root_by_day)]
	pub(super) type EvictedAnchorRoots<T: Config> =
		StorageMap<_, Blake2_256, u32, BoundedVec<u8, RootHashSize<T>>>;

	#[pallet::error]
	pub enum Error<T> {
		/// Anchor with anchor_id already exists
		AnchorAlreadyExists,

		/// Anchor store date must be in now or future
		AnchorStoreDateInPast,

		/// Anchor store date must not be more than max store date
		AnchorStoreDateAboveMaxLimit,

		/// Pre-commit already exists
		PreCommitAlreadyExists,

		/// Sender is not the owner of pre commit
		NotOwnerOfPreCommit,

		/// Invalid pre commit proof
		InvalidPreCommitProof,

		/// Eviction date too big for conversion
		EvictionDateTooBig,

		/// Failed to convert epoch in MS to days
		FailedToConvertEpochToDays,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Obtains an exclusive lock to make the next update to a certain
		/// document version identified by `anchor_id` on Centrifuge p2p network
		/// for a number of blocks given
		/// by [`PRE_COMMIT_EXPIRATION_DURATION_BLOCKS`] value. `signing_root`
		/// is a child node of the off-chain merkle tree of that document. In
		/// Centrifuge protocol, a document is committed only after reaching
		/// consensus with the other collaborators on the document. Consensus is
		/// reached by getting a cryptographic signature from other parties by
		/// sending them the `signing_root`. To deny the counter-party the free
		/// option of publishing its own state commitment upon receiving a
		/// request for signature, the node can first publish a pre-commit. Only
		/// the pre-committer account in the Centrifuge chain is allowed to
		/// `commit` a corresponding anchor before the pre-commit has expired.
		/// Some funds are reserved on a succesful pre-commit call.
		/// These funds are returned to the same account after a succesful
		/// [`Pallet::commit()`] call or explicitely if evicting the pre-commits
		/// by calling [`Pallet::evict_pre_commits()`]. For a more detailed
		/// explanation refer section 3.4 of [Centrifuge Protocol Paper](https://staticw.centrifuge.io/assets/centrifuge_os_protocol_paper.pdf)
		#[pallet::weight(<T as pallet::Config>::WeightInfo::pre_commit())]
		#[pallet::call_index(0)]
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
				Self::get_valid_pre_commit(anchor_id).is_none(),
				Error::<T>::PreCommitAlreadyExists
			);

			let expiration_block = <frame_system::Pallet<T>>::block_number()
				.checked_add(&BlockNumberFor::<T>::from(
					PRE_COMMIT_EXPIRATION_DURATION_BLOCKS,
				))
				.ok_or(ArithmeticError::Overflow)?;

			let deposit = T::Fees::fee_value(T::PreCommitDepositFeeKey::get());
			T::Currency::reserve(&who, deposit)?;

			<PreCommits<T>>::insert(
				anchor_id,
				PreCommitData {
					signing_root,
					identity: who.clone(),
					expiration_block,
					deposit,
				},
			);

			Ok(())
		}

		/// Commits a `document_root` of a merklized off chain document in
		/// Centrifuge p2p network as the latest version id(`anchor_id`)
		/// obtained by hashing `anchor_id_preimage`. If a pre-commit exists for
		/// the obtained `anchor_id`, hash of pre-committed `signing_root +
		/// proof` must match the given `doc_root`. Any pre-committed data is
		/// automatically removed on a succesful commit and the reserved
		/// funds from [`Pallet::pre_commit()`] are returned to the same
		/// account. To avoid state bloat on chain,
		/// the committed anchor would be evicted after the given
		/// `stored_until_date`. The calling account would be charged
		/// accordingly for the storage period. For a more detailed explanation
		/// refer section 3.4 of [Centrifuge Protocol Paper](https://staticw.centrifuge.io/assets/centrifuge_os_protocol_paper.pdf)
		#[pallet::weight(<T as pallet::Config>::WeightInfo::commit())]
		#[pallet::call_index(1)]
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

			let stored_until_date_from_epoch = common::get_days_since_epoch(eviction_date_u64)
				.ok_or(Error::<T>::EvictionDateTooBig)?;
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

			if let Some(pre_commit) = Self::get_valid_pre_commit(anchor_id) {
				ensure!(
					pre_commit.identity == who.clone(),
					Error::<T>::NotOwnerOfPreCommit
				);
				ensure!(
					Self::has_valid_pre_commit_proof(anchor_id, doc_root, proof),
					Error::<T>::InvalidPreCommitProof
				);
			}

			// pay the state rent
			let now_u64 = TryInto::<u64>::try_into(<pallet_timestamp::Pallet<T>>::get())
				.or(Err(ArithmeticError::Overflow))?;
			let today_in_days_from_epoch = common::get_days_since_epoch(now_u64)
				.ok_or(Error::<T>::FailedToConvertEpochToDays)?;

			let multiplier = stored_until_date_from_epoch
				.checked_sub(today_in_days_from_epoch)
				.ok_or(ArithmeticError::Underflow)?;

			// TODO(dev): move the fee to treasury account once its integrated instead of
			// burning fee we use the fee config setup on genesis for anchoring to calculate
			// the state rent
			let fee = T::Fees::fee_value(T::CommitAnchorFeeKey::get())
				.checked_mul(&multiplier.into())
				.ok_or(ArithmeticError::Overflow)?;

			// pay state rent to block author
			T::Fees::fee_to_author(&who, Fee::Balance(fee))?;

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

			Self::evict_pre_commit(anchor_id, false);

			Ok(())
		}

		/// Initiates eviction of pre-commits that has expired given a list on
		/// anchor ids. For each evicted pre-commits, the deposit holded by
		/// [`Pallet::pre_commit()`] call will be returned to the same account
		/// that made it originally.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::evict_pre_commits())]
		#[pallet::call_index(2)]
		pub fn evict_pre_commits(
			origin: OriginFor<T>,
			anchor_ids: BoundedVec<T::Hash, ConstU32<EVICT_PRE_COMMIT_LIST_SIZE>>,
		) -> DispatchResult {
			ensure_signed(origin)?;

			for anchor_id in anchor_ids {
				Self::evict_pre_commit(anchor_id, true);
			}

			Ok(())
		}

		/// Initiates eviction of expired anchors. Since anchors are stored on a
		/// child trie indexed by their eviction date, what this function does
		/// is to remove those child tries which has date_represented_by_root <
		/// current_date. Additionally it needs to take care of indexes
		/// created for accessing anchors, eg: to find an anchor given an id.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::evict_anchors())]
		#[pallet::call_index(3)]
		pub fn evict_anchors(origin: OriginFor<T>) -> DispatchResult {
			ensure_signed(origin)?;

			// get the today counting epoch, so that we can remove the corresponding child
			// trie
			let now_u64 = TryInto::<u64>::try_into(<pallet_timestamp::Pallet<T>>::get())
				.or(Err(ArithmeticError::Overflow))?;
			let today_in_days_from_epoch = common::get_days_since_epoch(now_u64)
				.ok_or(Error::<T>::FailedToConvertEpochToDays)?;

			let evict_date = <LatestEvictedDate<T>>::get()
				.unwrap_or_default()
				.checked_add(1)
				.ok_or(ArithmeticError::Overflow)?;

			// store yesterday as the last day of eviction
			let mut yesterday = today_in_days_from_epoch
				.checked_sub(1)
				.ok_or(ArithmeticError::Underflow)?;

			// Avoid to iterate more than 500 days
			if yesterday > evict_date + MAX_LOOP_IN_TX as u32 {
				yesterday = evict_date + MAX_LOOP_IN_TX as u32 - 1;
			}

			// remove child tries starting from day next to last evicted day
			let _evicted_trie_count =
				Self::evict_anchor_child_tries(evict_date, today_in_days_from_epoch);
			let _evicted_anchor_indexes_count = Self::remove_anchor_indexes(yesterday)?;
			<LatestEvictedDate<T>>::put(yesterday);

			Ok(())
		}
	}
	impl<T: Config> Pallet<T> {
		/// Checks if the given `anchor_id` has a valid pre-commit, i.e it has a
		/// pre-commit with `expiration_block` < `current_block_number`.
		fn get_valid_pre_commit(
			anchor_id: T::Hash,
		) -> Option<PreCommitData<T::Hash, T::AccountId, BlockNumberFor<T>, BalanceOf<T>>> {
			<PreCommits<T>>::get(anchor_id).filter(|pre_commit| {
				pre_commit.expiration_block > <frame_system::Pallet<T>>::block_number()
			})
		}

		/// Evict a pre-commit returning the reserved tokens at `pre_commit()`.
		/// You can choose if evict it only if it's expired or evict either way.
		fn evict_pre_commit(anchor_id: T::Hash, only_if_expired: bool) {
			if let Some(pre_commit) = PreCommits::<T>::get(anchor_id) {
				if !only_if_expired
					|| pre_commit.expiration_block <= <frame_system::Pallet<T>>::block_number()
				{
					PreCommits::<T>::remove(anchor_id);
					T::Currency::unreserve(&pre_commit.identity, pre_commit.deposit);
				}
			}
		}

		/// Checks if `hash(signing_root, proof) == doc_root` for the given
		/// `anchor_id`. Concatenation of `signing_root` and `proof` is done in
		/// a fixed order (signing_root + proof). assumes there is a valid
		/// pre-commit under PreCommits
		fn has_valid_pre_commit_proof(
			anchor_id: T::Hash,
			doc_root: T::Hash,
			proof: T::Hash,
		) -> bool {
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

		/// Remove child tries starting with `from` day to `until` day returning
		/// the count of tries removed.
		fn evict_anchor_child_tries(from: u32, until: u32) -> usize {
			(from..until)
				.map(|day| {
					(
						day,
						common::generate_child_storage_key(&Self::anchor_storage_key(
							&day.encode(),
						)),
					)
				})
				// store the root of child trie for the day on chain before eviction. Checks if it
				// exists before hand to ensure that it doesn't overwrite a root.
				.map(|(day, key)| {
					if !<EvictedAnchorRoots<T>>::contains_key(day) {
						let root: BoundedVec<_, _> = child::root(&key, StateVersion::V0)
							.try_into()
							.expect("The output hash must use the block hasher");

						<EvictedAnchorRoots<T>>::insert(day, root);
					}
					key
				})
				.map(|key| child::clear_storage(&key, None, None))
				.count()
		}

		/// Iterate from the last evicted anchor to latest anchor, while
		/// removing indexes that are no longer valid because they belong to an
		/// expired/evicted anchor. The loop is only allowed to run
		/// MAX_LOOP_IN_TX at a time.
		pub(crate) fn remove_anchor_indexes(yesterday: u32) -> Result<usize, DispatchError> {
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
					let anchor_id = <AnchorIndexes<T>>::get(idx)?;
					let eviction_date = <AnchorEvictDates<T>>::get(anchor_id).unwrap_or_default();
					Some((idx, anchor_id, eviction_date))
				})
				// filter out evictable anchors, anchor_evict_date can be 0 when evicting before any
				// anchors are created
				.filter(|(_, _, anchor_evict_date)| anchor_evict_date <= &yesterday)
				// remove indexes
				.map(|(idx, anchor_id, _)| {
					<AnchorEvictDates<T>>::remove(anchor_id);
					<AnchorIndexes<T>>::remove(idx);
					<LatestEvictedAnchorIndex<T>>::put(idx);
				})
				.count();
			Ok(count)
		}

		/// Get an anchor by its id in the child storage
		pub fn get_anchor_by_id(
			anchor_id: T::Hash,
		) -> Option<AnchorData<T::Hash, BlockNumberFor<T>>> {
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
	}
}
