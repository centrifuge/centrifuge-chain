//! A module for implementing Centrifuge document anchoring(merklized document commitments) on substrate for
//! Centrifuge chain.
//!
//! For a more formally detailed explanation refer section 3.4 of [Centrifuge Protocol Paper](https://staticw.centrifuge.io/assets/centrifuge_os_protocol_paper.pdf)

use crate::{common, fees};
use codec::{Decode, Encode};
use rstd::{convert::TryInto, vec::Vec};
use sr_primitives::{
    traits::Hash,
    weights::SimpleDispatchInfo,
};
use support::{decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageValue};
use system::ensure_signed;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

/// expiration duration in blocks of a pre-commit,
/// This is maximum expected time for document consensus to take place after a pre-commit of
/// an anchor and a commit to be received for the pre-committed anchor. Currently we expect to provide around 80mins for this.
/// Since our current block time as per chain_spec.rs is 10s, this means we have to provide 80 * 60 / 10 = 480 blocks of time for this.
const PRE_COMMIT_EXPIRATION_DURATION_BLOCKS: u64 = 480;

/// MUST be higher than 1 to assure that pre-commits are around during their validity time frame
/// The higher the number, the more pre-commits will be collected in a single eviction bucket
const PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER: u64 = 5;

/// Determines how many loop iterations are allowed to run at a time inside the runtime.
const MAX_LOOP_IN_TX: u64 = 500;

/// date 3000-01-01 -> 376200 days from unix epoch
const STORAGE_MAX_DAYS: u32 = 376200;

/// The data structure for storing pre-committed anchors.
#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PreCommitData<Hash, AccountId, BlockNumber> {
    signing_root: Hash,
    identity: AccountId,
    expiration_block: BlockNumber,
}

/// The data structure for storing committed anchors.
#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct AnchorData<Hash, BlockNumber> {
    id: Hash,
    doc_root: Hash,
    anchored_block: BlockNumber,
}

impl<Hash, BlockNumber> AnchorData<Hash, BlockNumber> {
    pub fn get_doc_root(self) -> Hash {
        self.doc_root
    }
}

/// The module's configuration trait.
pub trait Trait: system::Trait + timestamp::Trait + fees::Trait + balances::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_event! (
    pub enum Event<T>
    where
        <T as system::Trait>::Hash,
    {
        /// MoveAnchor event is triggered when the anchor should be moved to a different chain.
        /// AnchorID and its DocRoot are sent as part of the event.
        MoveAnchor(Hash, Hash),
    }
);

decl_storage! {
    trait Store for Module<T: Trait> as Anchor {

        /// PreCommits store the map of anchor Id to the pre-commit, which is a lock on an anchor id to be committed later
        PreCommits get(get_pre_commit): map T::Hash => PreCommitData<T::Hash, T::AccountId, T::BlockNumber>;

        /// Pre-commit eviction buckets keep track of which pre-commit can be evicted at which point
        PreCommitEvictionBuckets get(get_pre_commit_in_evict_bucket_by_index): map (T::BlockNumber, u64) => T::Hash;
        PreCommitEvictionBucketIndex get(get_pre_commits_count_in_evict_bucket): map T::BlockNumber => u64;

        /// index to find the eviction date given an anchor id
        AnchorEvictDates get(get_anchor_evict_date): map T::Hash => u32;

        /// incrementing index for anchors for iteration purposes
        AnchorIndexes get(get_anchor_id_by_index): map u64 => T::Hash;

        /// latest anchored index
        LatestAnchorIndex get(get_latest_anchor_index): u64;

        /// latest evicted anchor index. This would keep track of the latest evicted anchor index so
        /// that we can start the removal of AnchorEvictDates index from that index onwards. going
        /// from AnchorIndexes => AnchorEvictDates
        LatestEvictedAnchorIndex get(get_latest_evicted_anchor_index): u64;

        /// This is to keep track of the date when a child trie of anchors was evicted last. It is
        /// to evict historic anchor data child tries if they weren't evicted in a timely manner.
        LatestEvictedDate get(get_latest_evicted_date): u32;

        /// Storage for evicted anchor child trie roots. Anchors with a given expiry/eviction date
        /// are stored on-chain in a single child trie. This child trie is removed after the expiry
        /// date has passed while its root is stored permanently for proving an existence of an
        /// evicted anchor.
        EvictedAnchorRoots get(get_evicted_anchor_root_by_day): map u32 => Vec<u8>;

        Version: u64;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        fn deposit_event() = default;

        fn on_initialize(_now: T::BlockNumber) {
            if Version::get() == 0 {
                // do first upgrade
                // ...

                // uncomment when upgraded
                // <Version<T>>::put(1);
            }
        }

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
        /// For a more detailed explanation refer section 3.4 of [Centrifuge Protocol Paper](https://staticw.centrifuge.io/assets/centrifuge_os_protocol_paper.pdf)
        /// # <weight>
        /// minimal logic, also needs to be consume less block capacity + cheaper to make the pre-commits viable.
        /// # </weight>
        #[weight = SimpleDispatchInfo::FixedNormal(500_000)]
        pub fn pre_commit(origin, anchor_id: T::Hash, signing_root: T::Hash) -> Result {
            // TODO make payable
            let who = ensure_signed(origin)?;
            ensure!(Self::get_anchor_by_id(anchor_id).is_none(), "Anchor already exists");
            ensure!(!Self::has_valid_pre_commit(anchor_id), "A valid pre-commit already exists");

            let expiration_block = <system::Module<T>>::block_number()  +
                T::BlockNumber::from(Self::pre_commit_expiration_duration_blocks() as u32);
            <PreCommits<T>>::insert(anchor_id, PreCommitData {
                signing_root: signing_root,
                identity: who.clone(),
                expiration_block: expiration_block,
            });

            Self::put_pre_commit_into_eviction_bucket(anchor_id, expiration_block)?;

            Ok(())
        }

        /// Commits a `document_root` of a merklized off chain document in Centrifuge p2p network as
        /// the latest version id(`anchor_id`) obtained by hashing `anchor_id_preimage`. If a
        /// pre-commit exists for the obtained `anchor_id`, hash of pre-committed
        /// `signing_root + proof` must match the given `doc_root`. To avoid state bloat on chain,
        /// the committed anchor would be evicted after the given `stored_until_date`. The calling
        /// account would be charged accordingly for the storage period.
        /// For a more detailed explanation refer section 3.4 of [Centrifuge Protocol Paper](https://staticw.centrifuge.io/assets/centrifuge_os_protocol_paper.pdf)
        /// # <weight>
        /// State rent takes into account the storage cost depending on `stored_until_date`.
        /// Otherwise independant of the inputs. The weight cost is important as it helps avoid DOS
        /// using smaller `stored_until_date`s. Computation cost involves timestamp calculations
        /// and state rent calculations, which we take here to be equivalent to a transfer transaction.
        ///
        /// # </weight>
        #[weight = SimpleDispatchInfo::FixedNormal(1_000_000)]
        pub fn commit(origin, anchor_id_preimage: T::Hash, doc_root: T::Hash, proof: T::Hash, stored_until_date: T::Moment) -> Result {
            let who = ensure_signed(origin)?;
            ensure!(<timestamp::Module<T>>::get() + T::Moment::from(common::MS_PER_DAY.try_into().unwrap()) < stored_until_date,
                "Stored until date must be at least a day later than the current date");

            // validate the eviction date
            let eviction_date_u64 = TryInto::<u64>::try_into(stored_until_date)
                .map_err(|_e| "Can not convert eviction date to u64")
                .unwrap();
            let stored_until_date_from_epoch = common::get_days_since_epoch(eviction_date_u64);
            ensure!(Self::anchor_storage_max_days_from_now() >= stored_until_date_from_epoch, "The provided stored until date is more than the maximum allowed from now");

            let anchor_id = (anchor_id_preimage)
                .using_encoded(<T as system::Trait>::Hashing::hash);
            ensure!(Self::get_anchor_by_id(anchor_id).is_none(), "Anchor already exists");

            if Self::has_valid_pre_commit(anchor_id) {
                ensure!(<PreCommits<T>>::get(anchor_id).identity == who, "Pre-commit owned by someone else");
                ensure!(Self::has_valid_pre_commit_proof(anchor_id, doc_root, proof), "Pre-commit proof not valid");
            }

            let block_num = <system::Module<T>>::block_number();
            let child_storage_key = common::generate_child_storage_key(stored_until_date_from_epoch);
            let anchor_data = AnchorData {
                id: anchor_id,
                doc_root: doc_root,
                anchored_block: block_num
            };

            let anchor_data_encoded = anchor_data.encode();
            runtime_io::storage::child_set(&child_storage_key, anchor_id.as_ref(), &anchor_data_encoded);
            // update indexes
            <AnchorEvictDates<T>>::insert(&anchor_id, &stored_until_date_from_epoch);
            let idx = LatestAnchorIndex::get() + 1;
            <AnchorIndexes<T>>::insert(idx, &anchor_id);
            LatestAnchorIndex::put(idx);

            // pay the state rent
            let today_in_days_from_epoch = TryInto::<u64>::try_into(<timestamp::Module<T>>::get())
                .map(common::get_days_since_epoch)
                .map_err(|_e| "Can not convert timestamp to u64")
                .unwrap();

            // we use the fee config setup on genesis for anchoring to calculate the state rent
            let fee = <fees::Module<T>>::price_of(Self::fee_key()).unwrap() *
                <T as balances::Trait>::Balance::from(stored_until_date_from_epoch - today_in_days_from_epoch);
            <fees::Module<T>>::pay_fee_given(who, fee)?;

            Ok(())
        }

        /// Initiates eviction of pre-commits that has expired given that the current block number
        /// has progressed past the block number provided in `evict_bucket`. `evict_bucket` is also
        /// the index to find the pre-commits stored in storage to be evicted when the
        /// `evict_bucket` number of blocks has expired.
        /// # <weight>
        /// - discourage DoS
        /// # </weight>
        #[weight = SimpleDispatchInfo::FixedOperational(1_000_000)]
        pub fn evict_pre_commits(origin, evict_bucket: T::BlockNumber) -> Result {
            // TODO make payable
            ensure_signed(origin)?;
            ensure!(<system::Module<T>>::block_number() >= evict_bucket, "eviction only possible for bucket expiring < current block height");

            let pre_commits_count = Self::get_pre_commits_count_in_evict_bucket(evict_bucket);
            for idx in (0..pre_commits_count).rev() {
                if pre_commits_count - idx > MAX_LOOP_IN_TX {
                    break;
                }

                let pre_commit_id =
                    Self::get_pre_commit_in_evict_bucket_by_index((evict_bucket, idx));
                <PreCommits<T>>::remove(pre_commit_id);

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
        /// # <weight>
        /// - discourage DoS
        /// # </weight>
        #[weight = SimpleDispatchInfo::FixedOperational(1_000_000)]
        pub fn evict_anchors(origin) -> Result {
            ensure_signed(origin)?;
            let current_timestamp = <timestamp::Module<T>>::get();

            // get the today counting epoch, so that we can remove the corresponding child trie
            let today_in_days_from_epoch = TryInto::<u64>::try_into(current_timestamp)
                .map(common::get_days_since_epoch)
                .map_err(|_e| "Can not convert timestamp to u64")
                .unwrap();
            let evict_date = LatestEvictedDate::get();

            // remove child tries starting from day next to last evicted day
            let _evicted_trie_count = Self::evict_anchor_child_tries(evict_date + 1, today_in_days_from_epoch);

            // store yesterday as the last day of eviction
            let yesterday = today_in_days_from_epoch - 1;
            LatestEvictedDate::put(yesterday);
            let _evicted_anchor_indexes_count = Self::remove_anchor_indexes(yesterday);

            // TODO emit an event for this so that the process which triggers can know how many anchor indexes got purged

            Ok(())
        }

        /// Dispatch call when anchor by anchor_id is to be moved to another chain.
        /// TODO remove?
        #[weight = SimpleDispatchInfo::FixedNormal(1_000_000)]
        pub fn move_anchor(origin, anchor_id: T::Hash) -> Result {
            // ensure signed origin
            ensure_signed(origin)?;

            // fetch anchor data by anchor_id or fail if not present
            let anchor_data = Self::get_anchor_by_id(anchor_id).ok_or("Anchor doesn't exist")?;
            Self::deposit_event(RawEvent::MoveAnchor(anchor_data.id, anchor_data.doc_root));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// checks if the given `anchor_id` has a valid pre-commit, i.e it has a pre-commit with
    /// `expiration_block` < `current_block_number`.
    fn has_valid_pre_commit(anchor_id: T::Hash) -> bool {
        if !<PreCommits<T>>::exists(&anchor_id) {
            return false;
        }

        <PreCommits<T>>::get(anchor_id).expiration_block > <system::Module<T>>::block_number()
    }

    /// checks if `hash(signing_root, proof) == doc_root` for the given `anchor_id`. Concatenation
    /// of `signing_root` and `proof` is done in ascending order, according to the protocol defined
    /// by Centrifuge precise-proofs library for merklizing documents.
    fn has_valid_pre_commit_proof(anchor_id: T::Hash, doc_root: T::Hash, proof: T::Hash) -> bool {
        let signing_root = <PreCommits<T>>::get(anchor_id).signing_root;
        let mut signing_root_bytes = signing_root.as_ref().to_vec();
        let mut proof_bytes = proof.as_ref().to_vec();

        // order and concat hashes
        let concatenated_bytes: Vec<u8>;
        if signing_root_bytes < proof_bytes {
            signing_root_bytes.extend(proof_bytes);
            concatenated_bytes = signing_root_bytes;
        } else {
            proof_bytes.extend(signing_root_bytes);
            concatenated_bytes = proof_bytes;
        }

        let calculated_root = <T as system::Trait>::Hashing::hash(&concatenated_bytes);
        return doc_root == calculated_root;
    }

    /// TODO this needs to come from governance
    /// How long before we expire a pre-commit
    fn pre_commit_expiration_duration_blocks() -> u64 {
        PRE_COMMIT_EXPIRATION_DURATION_BLOCKS
    }

    /// Get the maximum days allowed for an anchor to be stored on chain from unix epoch onwards.
    /// TODO this needs to come from governance
    /// TODO this also needs to be calculated from a value from governance that provides the maximum days
    /// from now on instead of taking an absolute date from unix epoch through governance.
    fn anchor_storage_max_days_from_now() -> u32 {
        STORAGE_MAX_DAYS
    }

    /// Puts the pre-commit (based on anchor_id) into the correct eviction bucket
    fn put_pre_commit_into_eviction_bucket(
        anchor_id: T::Hash,
        expiration_block: T::BlockNumber,
    ) -> Result {
        // determine which eviction bucket to put into
        let evict_after_block = Self::determine_pre_commit_eviction_bucket(expiration_block);
        // get current index in eviction bucket and increment
        let mut eviction_bucket_size =
            Self::get_pre_commits_count_in_evict_bucket(evict_after_block);

        // add to eviction bucket and update bucket counter
        <PreCommitEvictionBuckets<T>>::insert(
            (evict_after_block.clone(), eviction_bucket_size.clone()),
            anchor_id,
        );
        eviction_bucket_size += 1;
        <PreCommitEvictionBucketIndex<T>>::insert(evict_after_block, eviction_bucket_size);
        Ok(())
    }

    /// Determines the next eviction bucket number based on the given BlockNumber
    /// This can be used to determine which eviction bucket a pre-commit
    /// should be put into for later eviction.
    /// TODO return err
    fn determine_pre_commit_eviction_bucket(
        pre_commit_expiration_block: T::BlockNumber,
    ) -> T::BlockNumber {
        let result = TryInto::<u32>::try_into(pre_commit_expiration_block);
        match result {
            Ok(u32_expiration_block) => {
                let expiration_horizon = Self::pre_commit_expiration_duration_blocks() as u32
                    * PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER as u32;
                let put_into_bucket = u32_expiration_block
                    - (u32_expiration_block % expiration_horizon)
                    + expiration_horizon;

                T::BlockNumber::from(put_into_bucket)
            }
            Err(_e) => T::BlockNumber::from(0),
        }
    }

    /// remove child tries starting with `from` day to `until` day returning the
    /// count of tries removed.
    fn evict_anchor_child_tries(from: u32, until: u32) -> usize {
        (from..until)
            .map(|day| (day, common::generate_child_storage_key(day)))
            // store the root of child trie for the day on chain before eviction. Checks if it
            // exists before hand to ensure that it doesn't overwrite a root.
            .map(|(day, key)| {
                if !EvictedAnchorRoots::exists(day) {
                    EvictedAnchorRoots::insert(day, runtime_io::storage::child_root(&key));
                }
                key
            })
            .map(|key| runtime_io::storage::child_storage_kill(&key))
            .count()
    }

    /// iterate from the last evicted anchor to latest anchor, while removing indexes that
    /// are no longer valid because they belong to an expired/evicted anchor. The loop is
    /// only allowed to run MAX_LOOP_IN_TX at a time.
    fn remove_anchor_indexes(yesterday: u32) -> usize {
        (LatestEvictedAnchorIndex::get() + 1..LatestAnchorIndex::get() + 1)
            // limit to only MAX_LOOP_IN_TX number of anchor indexes to remove
            .take(MAX_LOOP_IN_TX as usize)
            // get eviction date of the anchor given by index
            .map(|idx| {
                (
                    idx,
                    <AnchorEvictDates<T>>::get(<AnchorIndexes<T>>::get(idx)),
                )
            })
            // filter out evictable anchors, anchor_evict_date can be 0 when evicting before any anchors are created
            .filter(|(_, anchor_evict_date)| anchor_evict_date <= &yesterday)
            // remove indexes
            .map(|(idx, _)| {
                <AnchorEvictDates<T>>::remove(<AnchorIndexes<T>>::get(idx));
                <AnchorIndexes<T>>::remove(idx);
                LatestEvictedAnchorIndex::put(idx);
            })
            .count()
    }

    /// get an anchor by its id in the child storage
    pub fn get_anchor_by_id(anchor_id: T::Hash) -> Option<AnchorData<T::Hash, T::BlockNumber>> {
        let anchor_evict_date = <AnchorEvictDates<T>>::get(anchor_id);
        let storage_key = common::generate_child_storage_key(anchor_evict_date);

        runtime_io::storage::child_get(&storage_key, anchor_id.as_ref())
            .map(|data| AnchorData::decode(&mut &*data).ok().unwrap())
    }

    fn fee_key() -> <T as system::Trait>::Hash {
        <T as system::Trait>::Hashing::hash_of(&0)
    }
}

/// tests for anchor module
#[cfg(test)]
mod tests;
