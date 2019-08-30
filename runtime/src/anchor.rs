use codec::{Decode, Encode};
use app_crypto::RuntimeAppPublic;
use crate::opaque::{AnchorAuthorityId as AuthorityId};
use sr_primitives::{
    traits::{Extrinsic as ExtrinsicT, Hash}
};
use runtime_io::{Printable, print, submit_transaction, is_validator};
use rstd::prelude::*;
use rstd::vec::Vec;
use rstd::convert::TryInto;
use support::{decl_module, decl_storage, dispatch::Result, ensure, StorageMap, StorageValue};
use system::ensure_signed;

// expiration duration in blocks of a pre commit,
// This is maximum expected time for document consensus to take place after a pre-commit of
// an anchor and a commit to be received for the pre-committed anchor. Currently we expect to provide around 80mins for this.
// Since our current block time as per chain_spec.rs is 10s, this means we have to provide 80 * 60 / 10 = 480 blocks of time for this.
const PRE_COMMIT_EXPIRATION_DURATION_BLOCKS: u64 = 480;

// MUST be higher than 1 to assure that pre-commits are around during their validity timeframe
// The higher the number, the more pre-commits will be collected in a single eviction bucket
const PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER: u64 = 5;

// Determines how many pre-anchors are evicted at maximum per eviction TX
const PRE_COMMIT_EVICTION_MAX_LOOP_IN_TX: u64 = 500;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PreAnchorData<Hash, AccountId, BlockNumber> {
    signing_root: Hash,
    identity: AccountId,
    expiration_block: BlockNumber,
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct AnchorData<Hash, BlockNumber> {
    id: Hash,
    doc_root: Hash,
    anchored_block: BlockNumber,
}

/// The module's configuration trait.
pub trait Trait: system::Trait {

    /// The function call.
    type Call: From<Call<Self>>;

    /// A extrinsic right from the external world. This is unchecked and so
	/// can contain a signature.
    type UncheckedExtrinsic: ExtrinsicT<Call=<Self as Trait>::Call> + Encode + Decode;
}

decl_storage! {
    trait Store for Module<T: Trait> as Anchor {

        /// Pre Anchors store the map of anchor Id to the pre anchor, which is a lock on an anchor id to be committed later
        PreAnchors get(get_pre_anchor): map T::Hash => PreAnchorData<T::Hash, T::AccountId, T::BlockNumber>;

        /// Pre-anchor eviction buckets keep track of which pre-anchor can be evicted at which point
        PreAnchorEvictionBuckets get(get_pre_anchors_in_evict_bucket_by_index): map (T::BlockNumber, u64) => T::Hash;
        PreAnchorEvictionBucketIndex get(get_pre_anchors_count_in_evict_bucket): map T::BlockNumber => u64;

        /// Anchors store the map of anchor Id to the anchor
        Anchors get(get_anchor): map T::Hash => AnchorData<T::Hash, T::BlockNumber>;

        /// The current set of keys that can sign transactions on behalf of off-chain workers.
		SigningKeys get(keys): Vec<AuthorityId>;

        Version: u64;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        fn on_initialize(_now: T::BlockNumber) {
            if <Version>::get() == 0 {
                // do first upgrade
                // ...

                // uncomment when upgraded
                // <Version<T>>::put(1);
            }
        }

        pub fn pre_commit(origin, anchor_id: T::Hash, signing_root: T::Hash) -> Result {
            // TODO make payable
            let who = ensure_signed(origin)?;
            ensure!(!<Anchors<T>>::exists(anchor_id), "Anchor already exists");
            ensure!(!Self::has_valid_pre_commit(anchor_id), "A valid pre anchor already exists");

            let expiration_block = <system::Module<T>>::block_number()  + T::BlockNumber::from(Self::expiration_duration_blocks() as u32);
            <PreAnchors<T>>::insert(anchor_id, PreAnchorData {
                signing_root: signing_root,
                identity: who.clone(),
                expiration_block: expiration_block,
            });

            Self::put_pre_anchor_into_eviction_bucket(anchor_id)?;

            Ok(())
        }

        pub fn commit(origin, anchor_id_preimage: T::Hash, doc_root: T::Hash, proof: T::Hash) -> Result {
            // TODO make payable
            let who = ensure_signed(origin)?;

            let anchor_id = (anchor_id_preimage)
                .using_encoded(<T as system::Trait>::Hashing::hash);
            ensure!(!<Anchors<T>>::exists(anchor_id), "Anchor already exists");

            if Self::has_valid_pre_commit(anchor_id) {
                ensure!(<PreAnchors<T>>::get(anchor_id).identity == who, "Pre-commit owned by someone else");
                ensure!(Self::has_valid_pre_commit_proof(anchor_id, doc_root, proof), "Pre-commit proof not valid");
            }


            let block_num = <system::Module<T>>::block_number();
            <Anchors<T>>::insert(anchor_id, AnchorData {
                id: anchor_id,
                doc_root: doc_root,
                anchored_block: block_num,
            });

            Ok(())
        }

        pub fn evict_pre_commits(origin, evict_bucket: T::BlockNumber) -> Result {
            // TODO make payable
            ensure_signed(origin)?;
            ensure!((<system::Module<T>>::block_number() >= evict_bucket), "eviction only possible for bucket expiring < current block height");

            let pre_anchors_count = Self::get_pre_anchors_count_in_evict_bucket(evict_bucket);

            for idx in (0..pre_anchors_count).rev() {
                if pre_anchors_count - idx > PRE_COMMIT_EVICTION_MAX_LOOP_IN_TX {
                    break;
                }

                let pre_anchor_id = Self::get_pre_anchors_in_evict_bucket_by_index((evict_bucket, idx));
                <PreAnchors<T>>::remove(pre_anchor_id);

                <PreAnchorEvictionBuckets<T>>::remove((evict_bucket, idx));

                //decreases the evict bucket item count or remove index completely if empty
                if idx == 0 {
                    <PreAnchorEvictionBucketIndex<T>>::remove(evict_bucket);
                } else {

                    <PreAnchorEvictionBucketIndex<T>>::insert(evict_bucket, idx);
                }
            }
            Ok(())
        }

        fn offchain_worker(now: T::BlockNumber) {
            if is_validator() {
                match Self::evict_from_worker(now) {
                    Ok(_)  => {},
                    Err(e) => print(e),
                }
            }

        }
    }


}

impl<T: Trait> Module<T> {
    fn has_valid_pre_commit(anchor_id: T::Hash) -> bool {
        if !<PreAnchors<T>>::exists(&anchor_id) {
            return false;
        }

        <PreAnchors<T>>::get(anchor_id).expiration_block > <system::Module<T>>::block_number()
    }

    fn has_valid_pre_commit_proof(anchor_id: T::Hash, doc_root: T::Hash, proof: T::Hash) -> bool {
        let signing_root = <PreAnchors<T>>::get(anchor_id).signing_root;
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

    fn expiration_duration_blocks() -> u64 {
        // TODO this needs to come from governance
        PRE_COMMIT_EXPIRATION_DURATION_BLOCKS
    }

    // Puts the pre-anchor (based on anchor_id) into the correct eviction bucket
    fn put_pre_anchor_into_eviction_bucket(anchor_id: T::Hash) -> Result {
        // determine which eviction bucket to put into
        let evict_after_block =
            Self::determine_pre_anchor_eviction_bucket(<system::Module<T>>::block_number());
        // get current index in eviction bucket and increment
        let mut eviction_bucket_size =
            Self::get_pre_anchors_count_in_evict_bucket(evict_after_block);

        // add to eviction bucket and update bucket counter
        <PreAnchorEvictionBuckets<T>>::insert(
            (evict_after_block.clone(), eviction_bucket_size.clone()),
            anchor_id,
        );
        eviction_bucket_size += 1;
        <PreAnchorEvictionBucketIndex<T>>::insert(evict_after_block, eviction_bucket_size);
        Ok(())
    }

    // Determines the next eviction bucket number based on the given BlockNumber
    // This can be used to determine which eviction bucket a pre-commit
    // should be put into for later eviction.
    // TODO return err
    fn determine_pre_anchor_eviction_bucket(current_block: T::BlockNumber) -> T::BlockNumber {
        let result = TryInto::<u32>::try_into(current_block);
        match result {
            Ok(u32_current_block)  => {
                let expiration_horizon =
                    Self::expiration_duration_blocks() as u32 * PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER as u32;
                let put_into_bucket =
                    u32_current_block - (u32_current_block % expiration_horizon) + expiration_horizon;

                T::BlockNumber::from(put_into_bucket)
            },
            Err(_e) => T::BlockNumber::from(0),
        }
    }

    fn initialize_keys(keys: &[AuthorityId]) {
        if !keys.is_empty() {
            assert!(SigningKeys::get().is_empty(), "Keys are already initialized!");
            SigningKeys::put_ref(keys);
        }
    }

    fn evict_from_worker(block_number: T::BlockNumber) -> Result {
        // TODO sign and send evict tx
        Ok(())
    }
}

// implementing this trait allows us to listen to authority set changes.
impl<T: Trait> session::OneSessionHandler<T::AccountId> for Module<T> {

    type Key = AuthorityId;

    fn on_genesis_session<'a, I: 'a>(validators: I)
        where I: Iterator<Item=(&'a T::AccountId, AuthorityId)>
    {
        let keys = validators.map(|x| x.1).collect::<Vec<_>>();
        Self::initialize_keys(&keys);
    }

    fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, _queued_validators: I)
        where I: Iterator<Item=(&'a T::AccountId, AuthorityId)>
    {

        // Remember who the authorities are for the new session.
        SigningKeys::put(validators.map(|x| x.1).collect::<Vec<_>>());
    }

    fn on_before_session_ending() {

    }

    fn on_disabled(_i: usize) {
        // ignore
    }
}

/// tests for anchor module
#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Instant;
    use runtime_io::with_externalities;
    use primitives::{H256, Blake2Hasher};
    use support::{impl_outer_origin, assert_ok, assert_err, parameter_types};
    use sr_primitives::{
        generic,
        AnySignature,
        testing::Header,
        traits::{BlakeTwo256, IdentityLookup},
        Perbill,
        weights::Weight,
    };

    impl_outer_origin! {
		pub enum Origin for Test {}
	}

    // For testing the module, we construct most of a mock runtime. This means
    // first constructing a configuration type (`Test`) which `impl`s each of the
    // configuration traits of modules we want to use.
    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;
    parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaximumBlockWeight: Weight = 1024;
		pub const MaximumBlockLength: u32 = 2 * 1024;
		pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
	}
    impl system::Trait for Test {
        type Origin = Origin;
        type Call = ();
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type WeightMultiplierUpdate = ();
        type Event = ();
        type BlockHashCount = BlockHashCount;
        type MaximumBlockWeight = MaximumBlockWeight;
        type MaximumBlockLength = MaximumBlockLength;
        type AvailableBlockRatio = AvailableBlockRatio;
        type Version = ();
    }

    impl Trait for Test {
        type Call = Call<Self>;

        type UncheckedExtrinsic = generic::UncheckedExtrinsic<u64, Call<Self>, AnySignature, ()>;
    }

    impl Test {
        fn test_document_hashes() -> (
            <Test as system::Trait>::Hash,
            <Test as system::Trait>::Hash,
            <Test as system::Trait>::Hash,
        ) {
            // first is the hash of concatenated last two in sorted order
            (
                // doc_root
                [
                    86, 200, 105, 208, 164, 75, 251, 93, 233, 196, 84, 216, 68, 179, 91, 55, 113,
                    241, 229, 76, 16, 181, 40, 32, 205, 207, 120, 172, 147, 210, 53, 78,
                ]
                    .into(),
                // proof or signing root
                [
                    17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192,
                    142, 220, 157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218,
                ]
                    .into(),
                [
                    40, 156, 122, 201, 153, 204, 227, 25, 246, 138, 183, 211, 31, 191, 130, 124,
                    145, 37, 1, 1, 66, 168, 3, 230, 83, 111, 50, 108, 163, 179, 63, 52,
                ]
                    .into(),
            )
        }
    }

    type Anchor = Module<Test>;
    type System = system::Module<Test>;

    // This function basically just builds a genesis storage key/value store according to
    // our desired mockup.
    fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
        system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
    }

    #[test]
    fn basic_pre_commit() {
        with_externalities(&mut new_test_ext(), || {
            let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
            let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

            // reject unsigned
            assert_err!(
                Anchor::pre_commit(Origin::NONE, anchor_id, signing_root),
                "bad origin: expected to be a signed origin"
            );

            // happy
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id,
                signing_root
            ));
            // asserting that the stored pre anchor has the intended values set
            let a = Anchor::get_pre_anchor(anchor_id);
            assert_eq!(a.identity, 1);
            assert_eq!(a.signing_root, signing_root);
            assert_eq!(a.expiration_block, Anchor::expiration_duration_blocks() + 1);
        });
    }

    #[test]
    fn pre_commit_fail_anchor_exists() {
        with_externalities(&mut new_test_ext(), || {
            let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);
            // anchor
            assert_ok!(Anchor::commit(
                Origin::signed(1),
                pre_image,
                <Test as system::Trait>::Hashing::hash_of(&0),
                <Test as system::Trait>::Hashing::hash_of(&0)
            ));

            // fails because of existing anchor
            assert_err!(
                Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root),
                "Anchor already exists"
            );
        });
    }

    #[test]
    fn pre_commit_fail_anchor_exists_different_acc() {
        with_externalities(&mut new_test_ext(), || {
            let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);
            // anchor
            assert_ok!(Anchor::commit(
                Origin::signed(2),
                pre_image,
                <Test as system::Trait>::Hashing::hash_of(&0),
                <Test as system::Trait>::Hashing::hash_of(&0)
            ));

            // fails because of existing anchor
            assert_err!(
                Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root),
                "Anchor already exists"
            );
        });
    }

    #[test]
    fn pre_commit_fail_pre_anchor_exists() {
        with_externalities(&mut new_test_ext(), || {
            let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
            let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

            // first pre-anchor
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id,
                signing_root
            ));
            let a = Anchor::get_pre_anchor(anchor_id);
            assert_eq!(a.identity, 1);
            assert_eq!(a.signing_root, signing_root);
            assert_eq!(a.expiration_block, Anchor::expiration_duration_blocks() + 1);

            // fail, pre anchor exists
            assert_err!(
                Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root),
                "A valid pre anchor already exists"
            );

            // expire the pre commit
            System::set_block_number(Anchor::expiration_duration_blocks() + 2);
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id,
                signing_root
            ));
        });
    }

    #[test]
    fn pre_commit_fail_pre_anchor_exists_different_acc() {
        with_externalities(&mut new_test_ext(), || {
            let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
            let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

            // first pre-anchor
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id,
                signing_root
            ));
            let a = Anchor::get_pre_anchor(anchor_id);
            assert_eq!(a.identity, 1);
            assert_eq!(a.signing_root, signing_root);
            assert_eq!(a.expiration_block, Anchor::expiration_duration_blocks() + 1);

            // fail, pre anchor exists
            assert_err!(
                Anchor::pre_commit(Origin::signed(2), anchor_id, signing_root),
                "A valid pre anchor already exists"
            );

            // expire the pre commit
            System::set_block_number(Anchor::expiration_duration_blocks() + 2);
            assert_ok!(Anchor::pre_commit(
                Origin::signed(2),
                anchor_id,
                signing_root
            ));
        });
    }

    #[test]
    fn basic_commit() {
        with_externalities(&mut new_test_ext(), || {
            let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            let doc_root = <Test as system::Trait>::Hashing::hash_of(&0);
            // reject unsigned
            assert_err!(
                Anchor::commit(
                    Origin::NONE,
                    pre_image,
                    doc_root,
                    <Test as system::Trait>::Hashing::hash_of(&0)
                ),
                "bad origin: expected to be a signed origin"
            );

            // happy
            assert_ok!(Anchor::commit(
                Origin::signed(1),
                pre_image,
                doc_root,
                <Test as system::Trait>::Hashing::hash_of(&0)
            ));
            // asserting that the stored anchor id is what we sent the pre-image for
            let a = Anchor::get_anchor(anchor_id);
            assert_eq!(a.id, anchor_id);
            assert_eq!(a.doc_root, doc_root);
        });
    }

    #[test]
    fn commit_fail_anchor_exists() {
        with_externalities(&mut new_test_ext(), || {
            let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            let doc_root = <Test as system::Trait>::Hashing::hash_of(&0);

            // happy
            assert_ok!(Anchor::commit(
                Origin::signed(1),
                pre_image,
                doc_root,
                <Test as system::Trait>::Hashing::hash_of(&0)
            ));
            // asserting that the stored anchor id is what we sent the pre-image for
            let a = Anchor::get_anchor(anchor_id);
            assert_eq!(a.id, anchor_id);
            assert_eq!(a.doc_root, doc_root);

            assert_err!(
                Anchor::commit(
                    Origin::signed(1),
                    pre_image,
                    doc_root,
                    <Test as system::Trait>::Hashing::hash_of(&0)
                ),
                "Anchor already exists"
            );

            // different acc
            assert_err!(
                Anchor::commit(
                    Origin::signed(2),
                    pre_image,
                    doc_root,
                    <Test as system::Trait>::Hashing::hash_of(&0)
                ),
                "Anchor already exists"
            );
        });
    }

    #[test]
    fn basic_pre_commit_commit() {
        with_externalities(&mut new_test_ext(), || {
            let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            let random_doc_root = <Test as system::Trait>::Hashing::hash_of(&0);
            let (doc_root, signing_root, proof) = Test::test_document_hashes();

            // happy
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id,
                signing_root
            ));

            // wrong doc root
            assert_err!(
                Anchor::commit(Origin::signed(1), pre_image, random_doc_root, proof),
                "Pre-commit proof not valid"
            );

            // happy
            assert_ok!(Anchor::commit(
                Origin::signed(1),
                pre_image,
                doc_root,
                proof
            ));
            // asserting that the stored anchor id is what we sent the pre-image for
            let a = Anchor::get_anchor(anchor_id);
            assert_eq!(a.id, anchor_id);
            assert_eq!(a.doc_root, doc_root);

            // reverse order
            let pre_image = <Test as system::Trait>::Hashing::hash_of(&1);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            // reverse the proof and signing root hashes
            let (doc_root, proof, signing_root) = Test::test_document_hashes();

            // happy
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id,
                signing_root
            ));
            assert_ok!(Anchor::commit(
                Origin::signed(1),
                pre_image,
                doc_root,
                proof
            ));
        });
    }

    #[test]
    fn pre_commit_expired_when_anchoring() {
        with_externalities(&mut new_test_ext(), || {
            let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            let (doc_root, signing_root, proof) = Test::test_document_hashes();

            // happy
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id,
                signing_root
            ));
            // expire the pre commit
            System::set_block_number(Anchor::expiration_duration_blocks() + 2);

            // happy from a different account
            assert_ok!(Anchor::commit(
                Origin::signed(2),
                pre_image,
                doc_root,
                proof
            ));
            // asserting that the stored anchor id is what we sent the pre-image for
            let a = Anchor::get_anchor(anchor_id);
            assert_eq!(a.id, anchor_id);
            assert_eq!(a.doc_root, doc_root);
        });
    }

    #[test]
    fn pre_commit_commit_fail_from_another_acc() {
        with_externalities(&mut new_test_ext(), || {
            let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            let (doc_root, signing_root, proof) = Test::test_document_hashes();

            // happy
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id,
                signing_root
            ));

            // fail from a different account
            assert_err!(
                Anchor::commit(Origin::signed(2), pre_image, doc_root, proof),
                "Pre-commit owned by someone else"
            );
        });
    }

    // #### Pre Commit Eviction Tests
    #[test]
    fn pre_anchor_commit_bucket_gets_determined_correctly() {
        with_externalities(&mut new_test_ext(), || {
            let current_block: <Test as system::Trait>::BlockNumber = 1;
            let expected_evict_bucket: <Test as system::Trait>::BlockNumber =
                PRE_COMMIT_EXPIRATION_DURATION_BLOCKS * PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER;
            assert_eq!(
                expected_evict_bucket,
                Anchor::determine_pre_anchor_eviction_bucket(current_block)
            );

            let current_block2: <Test as system::Trait>::BlockNumber = expected_evict_bucket + 1;
            let expected_evict_bucket2: <Test as system::Trait>::BlockNumber =
                expected_evict_bucket * 2;
            assert_eq!(
                expected_evict_bucket2,
                Anchor::determine_pre_anchor_eviction_bucket(current_block2)
            );

            //testing with current bucket being even multiplier of EXPIRATION_DURATION_BLOCKS
            let current_block3: <Test as system::Trait>::BlockNumber = expected_evict_bucket2;
            let expected_evict_bucket3: <Test as system::Trait>::BlockNumber =
                expected_evict_bucket * 3;
            assert_eq!(
                expected_evict_bucket3,
                Anchor::determine_pre_anchor_eviction_bucket(current_block3)
            );
        });
    }

    #[test]
    fn put_pre_anchor_into_eviction_bucket_basic_pre_commit_eviction_bucket_registration() {
        with_externalities(&mut new_test_ext(), || {
            let anchor_id_0 = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id_1 = <Test as system::Trait>::Hashing::hash_of(&1);
            let anchor_id_2 = <Test as system::Trait>::Hashing::hash_of(&2);
            let anchor_id_3 = <Test as system::Trait>::Hashing::hash_of(&3);

            // three different block heights that will put anchors into different eviction buckets
            let block_height_0 = 1;
            let block_height_1 =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_0) + block_height_0;;
            let block_height_2 =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_1) + block_height_0;;


            // ------ First run ------
            // register anchor_id_0 into block_height_0
            System::set_block_number(block_height_0);
            assert_ok!(Anchor::put_pre_anchor_into_eviction_bucket(anchor_id_0));

            let mut current_pre_commit_evict_bucket =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_0);

            // asserting that the right bucket was used to store
            let mut pre_anchors_count =
                Anchor::get_pre_anchors_count_in_evict_bucket(current_pre_commit_evict_bucket);
            assert_eq!(pre_anchors_count, 1);
            let mut stored_pre_anchor_id = Anchor::get_pre_anchors_in_evict_bucket_by_index((
                current_pre_commit_evict_bucket,
                0,
            ));
            assert_eq!(stored_pre_anchor_id, anchor_id_0);

            // ------ Second run ------
            // register anchor_id_1 and anchor_id_2 into block_height_1
            System::set_block_number(block_height_1);
            assert_ok!(Anchor::put_pre_anchor_into_eviction_bucket(anchor_id_1));
            assert_ok!(Anchor::put_pre_anchor_into_eviction_bucket(anchor_id_2));

            current_pre_commit_evict_bucket =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_1);

            // asserting that the right bucket was used to store
            pre_anchors_count =
                Anchor::get_pre_anchors_count_in_evict_bucket(current_pre_commit_evict_bucket);
            assert_eq!(pre_anchors_count, 2);
            // first pre anchor
            stored_pre_anchor_id = Anchor::get_pre_anchors_in_evict_bucket_by_index((
                current_pre_commit_evict_bucket,
                0,
            ));
            assert_eq!(stored_pre_anchor_id, anchor_id_1);
            // second pre anchor
            stored_pre_anchor_id = Anchor::get_pre_anchors_in_evict_bucket_by_index((
                current_pre_commit_evict_bucket,
                1,
            ));
            assert_eq!(stored_pre_anchor_id, anchor_id_2);

            // ------ Third run ------
            // register anchor_id_3 into block_height_2
            System::set_block_number(block_height_2);
            assert_ok!(Anchor::put_pre_anchor_into_eviction_bucket(anchor_id_3));
            current_pre_commit_evict_bucket =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_2);

            // asserting that the right bucket was used to store
            pre_anchors_count =
                Anchor::get_pre_anchors_count_in_evict_bucket(current_pre_commit_evict_bucket);
            assert_eq!(pre_anchors_count, 1);
            stored_pre_anchor_id = Anchor::get_pre_anchors_in_evict_bucket_by_index((
                current_pre_commit_evict_bucket,
                0,
            ));
            assert_eq!(stored_pre_anchor_id, anchor_id_3);

            // finally a sanity check that the previous bucketed items are untouched by the subsequent runs
            // checking run #1 again
            current_pre_commit_evict_bucket =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_0);
            pre_anchors_count =
                Anchor::get_pre_anchors_count_in_evict_bucket(current_pre_commit_evict_bucket);
            assert_eq!(pre_anchors_count, 1);
            stored_pre_anchor_id = Anchor::get_pre_anchors_in_evict_bucket_by_index((
                current_pre_commit_evict_bucket,
                0,
            ));
            assert_eq!(stored_pre_anchor_id, anchor_id_0);
        });
    }

    #[test]
    fn pre_commit_with_pre_commit_eviction_bucket_registration() {
        with_externalities(&mut new_test_ext(), || {
            let anchor_id_0 = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id_1 = <Test as system::Trait>::Hashing::hash_of(&1);
            let anchor_id_2 = <Test as system::Trait>::Hashing::hash_of(&2);

            let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

            // three different block heights that will put anchors into different eviction buckets
            let block_height_0 = 1;
            let block_height_1 =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_0) + block_height_0;;


            // ------ Register the pre-commits ------
            // register anchor_id_0 into block_height_0
            System::set_block_number(block_height_0);
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id_0,
                signing_root
            ));

            System::set_block_number(block_height_1);
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id_1,
                signing_root
            ));
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id_2,
                signing_root
            ));

            // verify the pre-commits were registered
            // asserting that the stored pre anchor has the intended values set
            let pre_commit_0 = Anchor::get_pre_anchor(anchor_id_0);
            assert_eq!(pre_commit_0.identity, 1);
            assert_eq!(
                pre_commit_0.expiration_block,
                block_height_0 + Anchor::expiration_duration_blocks()
            );

            // verify the registration in evict bucket of anchor 0
            let mut pre_commit_evict_bucket =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_0);
            let pre_anchors_count =
                Anchor::get_pre_anchors_count_in_evict_bucket(pre_commit_evict_bucket);
            assert_eq!(pre_anchors_count, 1);
            let stored_pre_anchor_id =
                Anchor::get_pre_anchors_in_evict_bucket_by_index((pre_commit_evict_bucket, 0));
            assert_eq!(stored_pre_anchor_id, anchor_id_0);

            // verify the expected numbers on the evict bucket IDx
            pre_commit_evict_bucket = Anchor::determine_pre_anchor_eviction_bucket(block_height_1);
            assert_eq!(
                Anchor::get_pre_anchors_count_in_evict_bucket(pre_commit_evict_bucket),
                2
            );
        });
    }

    #[test]
    fn pre_commit_and_then_evict() {
        with_externalities(&mut new_test_ext(), || {
            let anchor_id_0 = <Test as system::Trait>::Hashing::hash_of(&0);
            let anchor_id_1 = <Test as system::Trait>::Hashing::hash_of(&1);
            let anchor_id_2 = <Test as system::Trait>::Hashing::hash_of(&2);

            let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

            // three different block heights that will put anchors into different eviction buckets
            let block_height_0 = 1;
            let block_height_1 =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_0) + block_height_0;;
            let block_height_2 =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_1) + block_height_0;;


            // ------ Register the pre-commits ------
            // register anchor_id_0 into block_height_0
            System::set_block_number(block_height_0);
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id_0,
                signing_root
            ));

            System::set_block_number(block_height_1);
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id_1,
                signing_root
            ));
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                anchor_id_2,
                signing_root
            ));

            // eviction fails within the "non evict time"
            System::set_block_number(
                Anchor::determine_pre_anchor_eviction_bucket(block_height_0) - 1,
            );
            assert_err!(
                Anchor::evict_pre_commits(
                    Origin::signed(1),
                    Anchor::determine_pre_anchor_eviction_bucket(block_height_0)
                ),
                "eviction only possible for bucket expiring < current block height"
            );

            // test that eviction works after expiration time
            System::set_block_number(block_height_2);
            let bucket_1 = Anchor::determine_pre_anchor_eviction_bucket(block_height_0);

            // before eviction, the pre-commit data findable
            let a = Anchor::get_pre_anchor(anchor_id_0);
            assert_eq!(a.identity, 1);
            assert_eq!(a.signing_root, signing_root);

            //do check counts, evict, check counts again
            assert_eq!(Anchor::get_pre_anchors_count_in_evict_bucket(bucket_1), 1);
            assert_ok!(Anchor::evict_pre_commits(Origin::signed(1), bucket_1));
            assert_eq!(Anchor::get_pre_anchors_count_in_evict_bucket(bucket_1), 0);

            // after eviction, the pre-commit data not findable
            let a_evicted = Anchor::get_pre_anchor(anchor_id_0);
            assert_eq!(a_evicted.identity, 0);
            assert_eq!(a_evicted.expiration_block, 0);

            let bucket_2 = Anchor::determine_pre_anchor_eviction_bucket(block_height_1);
            assert_eq!(Anchor::get_pre_anchors_count_in_evict_bucket(bucket_2), 2);
            assert_ok!(Anchor::evict_pre_commits(Origin::signed(1), bucket_2));
            assert_eq!(Anchor::get_pre_anchors_count_in_evict_bucket(bucket_2), 0);
        });
    }

    #[test]
    fn pre_commit_and_then_evict_larger_than_max_evict() {
        with_externalities(&mut new_test_ext(), || {
            let block_height_0 = 1;
            let block_height_1 =
                Anchor::determine_pre_anchor_eviction_bucket(block_height_0) + block_height_0;
            let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

            System::set_block_number(block_height_0);
            for idx in 0..PRE_COMMIT_EVICTION_MAX_LOOP_IN_TX + 6 {
                assert_ok!(Anchor::pre_commit(
                    Origin::signed(1),
                    <Test as system::Trait>::Hashing::hash_of(&idx),
                    signing_root
                ));
            }

            System::set_block_number(block_height_1);
            let bucket_1 = Anchor::determine_pre_anchor_eviction_bucket(block_height_0);

            //do check counts, evict, check counts again
            assert_eq!(Anchor::get_pre_anchors_count_in_evict_bucket(bucket_1), 506);
            assert_ok!(Anchor::evict_pre_commits(Origin::signed(1), bucket_1));
            assert_eq!(Anchor::get_pre_anchors_count_in_evict_bucket(bucket_1), 6);

            // evict again, now should be empty
            System::set_block_number(block_height_1 + 1);
            assert_ok!(Anchor::evict_pre_commits(Origin::signed(1), bucket_1));
            assert_eq!(Anchor::get_pre_anchors_count_in_evict_bucket(bucket_1), 0);
        });
    }
    // #### End Pre Commit Eviction Tests

    #[test]
    #[ignore]
    fn basic_commit_perf() {
        with_externalities(&mut new_test_ext(), || {
            let mut elapsed: u128 = 0;
            for i in 0..100000 {
                let random_seed = <system::Module<Test>>::random_seed();
                let pre_image =
                    (random_seed, i).using_encoded(<Test as system::Trait>::Hashing::hash);
                let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
                let (doc_root, signing_root, proof) = Test::test_document_hashes();

                // happy
                assert_ok!(Anchor::pre_commit(
                    Origin::signed(1),
                    anchor_id,
                    signing_root
                ));

                let now = Instant::now();

                assert_ok!(Anchor::commit(
                    Origin::signed(1),
                    pre_image,
                    doc_root,
                    proof
                ));

                elapsed = elapsed + now.elapsed().as_micros();
            }

            println!("time {}", elapsed);
        });
    }
}
