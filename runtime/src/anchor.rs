use parity_codec::{Decode, Encode};
use rstd::vec::Vec;
use runtime_primitives::traits::{As, Hash};
use support::{decl_module, decl_storage, dispatch::Result, ensure, StorageMap};
use system::ensure_signed;

// expiration duration in blocks of a pre commit,
// This is maximum expected time for document consensus to take place after a pre-commit of
// an anchor and a commit to be received for the pre-committed anchor. Currently we expect to provide around 80mins for this.
// Since our current block time as per chain_spec.rs is 10s, this means we have to provide 80 * 60 / 10 = 480 blocks of time for this.
const EXPIRATION_DURATION_BLOCKS: u64 = 480;

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
pub trait Trait: system::Trait {}

decl_storage! {
    trait Store for Module<T: Trait> as Anchor {

        // Pre Anchors store the map of anchor Id to the pre anchor, which is a lock on an anchor id to be committed later
        PreAnchors get(get_pre_anchor): map T::Hash => PreAnchorData<T::Hash, T::AccountId, T::BlockNumber>;

        // Anchors store the map of anchor Id to the anchor
        Anchors get(get_anchor): map T::Hash => AnchorData<T::Hash, T::BlockNumber>;

        Version: u64;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        fn on_initialize(now: T::BlockNumber) {
            if <Version<T>>::get() == 0 {
                // do first upgrade
                // ...

                // uncomment when upgraded
                // <Version<T>>::put(1);
            }
        }

        pub fn pre_commit(origin, anchor_id: T::Hash, signing_root: T::Hash) -> Result {
            let who = ensure_signed(origin)?;
            ensure!(!<Anchors<T>>::exists(anchor_id), "Anchor already exists");
            ensure!(!Self::has_valid_pre_commit(anchor_id), "A valid pre anchor already exists");

            let expiration_block = <system::Module<T>>::block_number()  + As::sa(Self::expiration_duration_blocks());
            <PreAnchors<T>>::insert(anchor_id, PreAnchorData {
                signing_root: signing_root,
                identity: who.clone(),
                expiration_block: expiration_block,
            });

            Ok(())
        }

        pub fn commit(origin, anchor_id_preimage: T::Hash, doc_root: T::Hash, proof: T::Hash) -> Result {
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
        EXPIRATION_DURATION_BLOCKS
    }
}

/// tests for anchor module
#[cfg(test)]
mod tests {
    use primitives::{Blake2Hasher, H256};
    use runtime_io::with_externalities;
    use runtime_primitives::{
        testing::{Digest, DigestItem, Header},
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage,
    };
    use std::time::Instant;
    use support::{assert_err, assert_ok, impl_outer_origin};

    use super::*;

    impl_outer_origin! {
        pub enum Origin for Test {}
    }

    // For testing the module, we construct most of a mock runtime. This means
    // first constructing a configuration type (`Test`) which `impl`s each of the
    // configuration traits of modules we want to use.
    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;
    impl system::Trait for Test {
        type Origin = Origin;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type Digest = Digest;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = ();
        type Log = DigestItem;
    }
    impl Trait for Test {}

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
        system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap()
            .0
            .into()
    }

    #[test]
    fn basic_pre_commit() {
        with_externalities(&mut new_test_ext(), || {
            let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
            let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

            // reject unsigned
            assert_err!(
                Anchor::pre_commit(Origin::INHERENT, anchor_id, signing_root),
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
                    Origin::INHERENT,
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
