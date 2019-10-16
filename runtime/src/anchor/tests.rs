use super::*;

use primitives::H256;
use sr_primitives::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    weights::Weight,
    Perbill,
};
use std::time::Instant;
use support::{assert_err, assert_ok, impl_outer_origin, parameter_types, traits::Randomness};

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

impl timestamp::Trait for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ();
}

impl fees::Trait for Test {
    type Event = ();
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 0;
    pub const TransferFee: u64 = 0;
    pub const CreationFee: u64 = 0;
    pub const TransactionBaseFee: u64 = 0;
    pub const TransactionByteFee: u64 = 0;
}
impl balances::Trait for Test {
    type Balance = u64;
    type OnFreeBalanceZero = ();
    type OnNewAccount = ();
    type Event = ();
    type TransactionPayment = ();
    type TransferPayment = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type TransferFee = TransferFee;
    type CreationFee = CreationFee;
    type TransactionBaseFee = TransactionBaseFee;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = ();
}

impl Trait for Test {
    type Event = ();
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
                86, 200, 105, 208, 164, 75, 251, 93, 233, 196, 84, 216, 68, 179, 91, 55, 113, 241,
                229, 76, 16, 181, 40, 32, 205, 207, 120, 172, 147, 210, 53, 78,
            ]
            .into(),
            // proof or signing root
            [
                17, 192, 231, 155, 113, 195, 151, 108, 205, 12, 2, 209, 49, 14, 37, 22, 192, 142,
                220, 157, 139, 111, 87, 204, 214, 128, 214, 58, 77, 142, 114, 218,
            ]
            .into(),
            [
                40, 156, 122, 201, 153, 204, 227, 25, 246, 138, 183, 211, 31, 191, 130, 124, 145,
                37, 1, 1, 66, 168, 3, 230, 83, 111, 50, 108, 163, 179, 63, 52,
            ]
            .into(),
        )
    }
}

type Anchor = Module<Test>;
type System = system::Module<Test>;

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
fn new_test_ext() -> runtime_io::TestExternalities {
    let mut t = system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();
    fees::GenesisConfig::<Test> {
        initial_fees: vec![(
            // anchoring state rent fee per day
            H256::from(&[
                17, 218, 109, 31, 118, 29, 223, 155, 219, 76, 157, 110, 83, 3, 235, 212, 31, 97,
                133, 141, 10, 86, 71, 161, 167, 191, 224, 137, 191, 146, 27, 233,
            ]),
            // state rent 0 for tests
            0,
        )],
    }
    .assimilate_storage(&mut t)
    .unwrap();
    t.into()
}

#[test]
fn basic_pre_commit() {
    new_test_ext().execute_with(|| {
        let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
        let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

        // reject unsigned
        assert_err!(
            Anchor::pre_commit(Origin::NONE, anchor_id, signing_root),
            "RequireSignedOrigin"
        );

        // happy
        assert_ok!(Anchor::pre_commit(
            Origin::signed(1),
            anchor_id,
            signing_root
        ));
        // asserting that the stored pre-commit has the intended values set
        let a = Anchor::get_pre_commit(anchor_id);
        assert_eq!(a.identity, 1);
        assert_eq!(a.signing_root, signing_root);
        assert_eq!(
            a.expiration_block,
            Anchor::pre_commit_expiration_duration_blocks() + 1
        );
    });
}

#[test]
fn pre_commit_fail_anchor_exists() {
    new_test_ext().execute_with(|| {
        let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
        let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);
        // anchor
        assert_ok!(Anchor::commit(
            Origin::signed(1),
            pre_image,
            <Test as system::Trait>::Hashing::hash_of(&0),
            <Test as system::Trait>::Hashing::hash_of(&0),
            common::MS_PER_DAY + 1
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
    new_test_ext().execute_with(|| {
        let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
        let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);
        // anchor
        assert_ok!(Anchor::commit(
            Origin::signed(2),
            pre_image,
            <Test as system::Trait>::Hashing::hash_of(&0),
            <Test as system::Trait>::Hashing::hash_of(&0),
            common::MS_PER_DAY + 1
        ));

        // fails because of existing anchor
        assert_err!(
            Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root),
            "Anchor already exists"
        );
    });
}

#[test]
fn pre_commit_fail_pre_commit_exists() {
    new_test_ext().execute_with(|| {
        let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
        let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

        // first pre-commit
        assert_ok!(Anchor::pre_commit(
            Origin::signed(1),
            anchor_id,
            signing_root
        ));
        let a = Anchor::get_pre_commit(anchor_id);
        assert_eq!(a.identity, 1);
        assert_eq!(a.signing_root, signing_root);
        assert_eq!(
            a.expiration_block,
            Anchor::pre_commit_expiration_duration_blocks() + 1
        );

        // fail, pre-commit exists
        assert_err!(
            Anchor::pre_commit(Origin::signed(1), anchor_id, signing_root),
            "A valid pre-commit already exists"
        );

        // expire the pre-commit
        System::set_block_number(Anchor::pre_commit_expiration_duration_blocks() + 2);
        assert_ok!(Anchor::pre_commit(
            Origin::signed(1),
            anchor_id,
            signing_root
        ));
    });
}

#[test]
fn pre_commit_fail_pre_commit_exists_different_acc() {
    new_test_ext().execute_with(|| {
        let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
        let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

        // first pre-commit
        assert_ok!(Anchor::pre_commit(
            Origin::signed(1),
            anchor_id,
            signing_root
        ));
        let a = Anchor::get_pre_commit(anchor_id);
        assert_eq!(a.identity, 1);
        assert_eq!(a.signing_root, signing_root);
        assert_eq!(
            a.expiration_block,
            Anchor::pre_commit_expiration_duration_blocks() + 1
        );

        // fail, pre-commit exists
        assert_err!(
            Anchor::pre_commit(Origin::signed(2), anchor_id, signing_root),
            "A valid pre-commit already exists"
        );

        // expire the pre-commit
        System::set_block_number(Anchor::pre_commit_expiration_duration_blocks() + 2);
        assert_ok!(Anchor::pre_commit(
            Origin::signed(2),
            anchor_id,
            signing_root
        ));
    });
}

#[test]
fn basic_commit() {
    new_test_ext().execute_with(|| {
        let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
        let pre_image2 = <Test as system::Trait>::Hashing::hash_of(&1);
        let anchor_id2 = (pre_image2).using_encoded(<Test as system::Trait>::Hashing::hash);
        let doc_root = <Test as system::Trait>::Hashing::hash_of(&0);
        // reject unsigned
        assert_err!(
            Anchor::commit(
                Origin::NONE,
                pre_image,
                doc_root,
                <Test as system::Trait>::Hashing::hash_of(&0),
                1
            ),
            "RequireSignedOrigin"
        );

        // happy
        assert_ok!(Anchor::commit(
            Origin::signed(1),
            pre_image,
            doc_root,
            <Test as system::Trait>::Hashing::hash_of(&0),
            1567589834087
        ));
        // asserting that the stored anchor id is what we sent the pre-image for
        let mut a = Anchor::get_anchor_by_id(anchor_id).unwrap();
        assert_eq!(a.id, anchor_id);
        assert_eq!(a.doc_root, doc_root);
        assert_eq!(Anchor::get_anchor_evict_date(anchor_id), 18144);
        assert_eq!(
            Anchor::get_anchor_id_by_index(Anchor::get_latest_anchor_index()),
            anchor_id
        );
        assert_eq!(Anchor::get_anchor_id_by_index(1), anchor_id);

        // commit second anchor to test index updates
        assert_ok!(Anchor::commit(
            Origin::signed(1),
            pre_image2,
            doc_root,
            <Test as system::Trait>::Hashing::hash_of(&0),
            1567589844087
        ));
        a = Anchor::get_anchor_by_id(anchor_id2).unwrap();
        assert_eq!(a.id, anchor_id2);
        assert_eq!(a.doc_root, doc_root);
        assert_eq!(Anchor::get_anchor_evict_date(anchor_id2), 18144);
        assert_eq!(Anchor::get_anchor_id_by_index(2), anchor_id2);
        assert_eq!(
            Anchor::get_anchor_id_by_index(Anchor::get_latest_anchor_index()),
            anchor_id2
        );

        // commit anchor with a less than required number of minimum storage days
        assert_err!(
            Anchor::commit(
                Origin::signed(1),
                pre_image2,
                doc_root,
                <Test as system::Trait>::Hashing::hash_of(&0),
                2 // some arbitrary store until date that is less than the required minimum
            ),
            "Stored until date must be at least a day later than the current date"
        );
    });
}

#[test]
fn commit_fail_anchor_exists() {
    new_test_ext().execute_with(|| {
        let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
        let doc_root = <Test as system::Trait>::Hashing::hash_of(&0);

        // happy
        assert_ok!(Anchor::commit(
            Origin::signed(1),
            pre_image,
            doc_root,
            <Test as system::Trait>::Hashing::hash_of(&0),
            common::MS_PER_DAY + 1
        ));
        // asserting that the stored anchor id is what we sent the pre-image for
        let a = Anchor::get_anchor_by_id(anchor_id).unwrap();
        assert_eq!(a.id, anchor_id);
        assert_eq!(a.doc_root, doc_root);

        assert_err!(
            Anchor::commit(
                Origin::signed(1),
                pre_image,
                doc_root,
                <Test as system::Trait>::Hashing::hash_of(&0),
                common::MS_PER_DAY + 1
            ),
            "Anchor already exists"
        );

        // different acc
        assert_err!(
            Anchor::commit(
                Origin::signed(2),
                pre_image,
                doc_root,
                <Test as system::Trait>::Hashing::hash_of(&0),
                common::MS_PER_DAY + 1
            ),
            "Anchor already exists"
        );
    });
}

#[test]
fn basic_pre_commit_commit() {
    new_test_ext().execute_with(|| {
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
            Anchor::commit(
                Origin::signed(1),
                pre_image,
                random_doc_root,
                proof,
                common::MS_PER_DAY + 1
            ),
            "Pre-commit proof not valid"
        );

        // happy
        assert_ok!(Anchor::commit(
            Origin::signed(1),
            pre_image,
            doc_root,
            proof,
            common::MS_PER_DAY + 1
        ));
        // asserting that the stored anchor id is what we sent the pre-image for
        let a = Anchor::get_anchor_by_id(anchor_id).unwrap();
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
            proof,
            common::MS_PER_DAY + 1
        ));
    });
}

#[test]
fn pre_commit_expired_when_anchoring() {
    new_test_ext().execute_with(|| {
        let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
        let (doc_root, signing_root, proof) = Test::test_document_hashes();

        // happy
        assert_ok!(Anchor::pre_commit(
            Origin::signed(1),
            anchor_id,
            signing_root
        ));
        // expire the pre-commit
        System::set_block_number(Anchor::pre_commit_expiration_duration_blocks() + 2);

        // happy from a different account
        assert_ok!(Anchor::commit(
            Origin::signed(2),
            pre_image,
            doc_root,
            proof,
            common::MS_PER_DAY + 1
        ));
        // asserting that the stored anchor id is what we sent the pre-image for
        let a = Anchor::get_anchor_by_id(anchor_id).unwrap();
        assert_eq!(a.id, anchor_id);
        assert_eq!(a.doc_root, doc_root);
    });
}

#[test]
fn pre_commit_commit_fail_from_another_acc() {
    new_test_ext().execute_with(|| {
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
            Anchor::commit(
                Origin::signed(2),
                pre_image,
                doc_root,
                proof,
                common::MS_PER_DAY + 1
            ),
            "Pre-commit owned by someone else"
        );
    });
}

// #### Pre Commit Eviction Tests
#[test]
fn pre_commit_commit_bucket_gets_determined_correctly() {
    new_test_ext().execute_with(|| {
        let current_block: <Test as system::Trait>::BlockNumber = 1;
        let expected_evict_bucket: <Test as system::Trait>::BlockNumber =
            PRE_COMMIT_EXPIRATION_DURATION_BLOCKS * PRE_COMMIT_EVICTION_BUCKET_MULTIPLIER;
        assert_eq!(
            expected_evict_bucket,
            Anchor::determine_pre_commit_eviction_bucket(current_block)
        );

        let current_block2: <Test as system::Trait>::BlockNumber = expected_evict_bucket + 1;
        let expected_evict_bucket2: <Test as system::Trait>::BlockNumber =
            expected_evict_bucket * 2;
        assert_eq!(
            expected_evict_bucket2,
            Anchor::determine_pre_commit_eviction_bucket(current_block2)
        );

        //testing with current bucket being even multiplier of EXPIRATION_DURATION_BLOCKS
        let current_block3: <Test as system::Trait>::BlockNumber = expected_evict_bucket2;
        let expected_evict_bucket3: <Test as system::Trait>::BlockNumber =
            expected_evict_bucket * 3;
        assert_eq!(
            expected_evict_bucket3,
            Anchor::determine_pre_commit_eviction_bucket(current_block3)
        );
    });
}

#[test]
fn put_pre_commit_into_eviction_bucket_basic_pre_commit_eviction_bucket_registration() {
    new_test_ext().execute_with(|| {
        let anchor_id_0 = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id_1 = <Test as system::Trait>::Hashing::hash_of(&1);
        let anchor_id_2 = <Test as system::Trait>::Hashing::hash_of(&2);
        let anchor_id_3 = <Test as system::Trait>::Hashing::hash_of(&3);

        // three different block heights that will put anchors into different eviction buckets
        let block_height_0 = 1;
        let block_height_1 =
            Anchor::determine_pre_commit_eviction_bucket(block_height_0) + block_height_0;
        let block_height_2 =
            Anchor::determine_pre_commit_eviction_bucket(block_height_1) + block_height_0;

        // ------ First run ------
        // register anchor_id_0 into block_height_0
        System::set_block_number(block_height_0);
        assert_ok!(Anchor::put_pre_commit_into_eviction_bucket(
            anchor_id_0,
            block_height_0
        ));

        let mut current_pre_commit_evict_bucket =
            Anchor::determine_pre_commit_eviction_bucket(block_height_0);

        // asserting that the right bucket was used to store
        let mut pre_commits_count =
            Anchor::get_pre_commits_count_in_evict_bucket(current_pre_commit_evict_bucket);
        assert_eq!(pre_commits_count, 1);
        let mut stored_pre_commit_id =
            Anchor::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 0));
        assert_eq!(stored_pre_commit_id, anchor_id_0);

        // ------ Second run ------
        // register anchor_id_1 and anchor_id_2 into block_height_1
        System::set_block_number(block_height_1);
        assert_ok!(Anchor::put_pre_commit_into_eviction_bucket(
            anchor_id_1,
            block_height_1
        ));
        assert_ok!(Anchor::put_pre_commit_into_eviction_bucket(
            anchor_id_2,
            block_height_1
        ));

        current_pre_commit_evict_bucket =
            Anchor::determine_pre_commit_eviction_bucket(block_height_1);

        // asserting that the right bucket was used to store
        pre_commits_count =
            Anchor::get_pre_commits_count_in_evict_bucket(current_pre_commit_evict_bucket);
        assert_eq!(pre_commits_count, 2);
        // first pre-commit
        stored_pre_commit_id =
            Anchor::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 0));
        assert_eq!(stored_pre_commit_id, anchor_id_1);
        // second pre-commit
        stored_pre_commit_id =
            Anchor::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 1));
        assert_eq!(stored_pre_commit_id, anchor_id_2);

        // ------ Third run ------
        // register anchor_id_3 into block_height_2
        System::set_block_number(block_height_2);
        assert_ok!(Anchor::put_pre_commit_into_eviction_bucket(
            anchor_id_3,
            block_height_2
        ));
        current_pre_commit_evict_bucket =
            Anchor::determine_pre_commit_eviction_bucket(block_height_2);

        // asserting that the right bucket was used to store
        pre_commits_count =
            Anchor::get_pre_commits_count_in_evict_bucket(current_pre_commit_evict_bucket);
        assert_eq!(pre_commits_count, 1);
        stored_pre_commit_id =
            Anchor::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 0));
        assert_eq!(stored_pre_commit_id, anchor_id_3);

        // finally a sanity check that the previous bucketed items are untouched by the subsequent runs
        // checking run #1 again
        current_pre_commit_evict_bucket =
            Anchor::determine_pre_commit_eviction_bucket(block_height_0);
        pre_commits_count =
            Anchor::get_pre_commits_count_in_evict_bucket(current_pre_commit_evict_bucket);
        assert_eq!(pre_commits_count, 1);
        stored_pre_commit_id =
            Anchor::get_pre_commit_in_evict_bucket_by_index((current_pre_commit_evict_bucket, 0));
        assert_eq!(stored_pre_commit_id, anchor_id_0);
    });
}

#[test]
fn pre_commit_with_pre_commit_eviction_bucket_registration() {
    new_test_ext().execute_with(|| {
        let anchor_id_0 = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id_1 = <Test as system::Trait>::Hashing::hash_of(&1);
        let anchor_id_2 = <Test as system::Trait>::Hashing::hash_of(&2);

        let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

        // three different block heights that will put anchors into different eviction buckets
        let block_height_0 = 1;
        let block_height_1 =
            Anchor::determine_pre_commit_eviction_bucket(block_height_0) + block_height_0;;


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
        // asserting that the stored pre-commit has the intended values set
        let pre_commit_0 = Anchor::get_pre_commit(anchor_id_0);
        assert_eq!(pre_commit_0.identity, 1);
        assert_eq!(
            pre_commit_0.expiration_block,
            block_height_0 + Anchor::pre_commit_expiration_duration_blocks()
        );

        // verify the registration in evict bucket of anchor 0
        let mut pre_commit_evict_bucket =
            Anchor::determine_pre_commit_eviction_bucket(block_height_0);
        let pre_commits_count =
            Anchor::get_pre_commits_count_in_evict_bucket(pre_commit_evict_bucket);
        assert_eq!(pre_commits_count, 1);
        let stored_pre_commit_id =
            Anchor::get_pre_commit_in_evict_bucket_by_index((pre_commit_evict_bucket, 0));
        assert_eq!(stored_pre_commit_id, anchor_id_0);

        // verify the expected numbers on the evict bucket IDx
        pre_commit_evict_bucket = Anchor::determine_pre_commit_eviction_bucket(block_height_1);
        assert_eq!(
            Anchor::get_pre_commits_count_in_evict_bucket(pre_commit_evict_bucket),
            2
        );
    });
}

#[test]
fn pre_commit_and_then_evict() {
    new_test_ext().execute_with(|| {
        let anchor_id_0 = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id_1 = <Test as system::Trait>::Hashing::hash_of(&1);
        let anchor_id_2 = <Test as system::Trait>::Hashing::hash_of(&2);

        let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

        // three different block heights that will put anchors into different eviction buckets
        let block_height_0 = 1;
        let block_height_1 =
            Anchor::determine_pre_commit_eviction_bucket(block_height_0) + block_height_0;;
        let block_height_2 =
            Anchor::determine_pre_commit_eviction_bucket(block_height_1) + block_height_0;;


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
        System::set_block_number(Anchor::determine_pre_commit_eviction_bucket(block_height_0) - 1);
        assert_err!(
            Anchor::evict_pre_commits(
                Origin::signed(1),
                Anchor::determine_pre_commit_eviction_bucket(block_height_0)
            ),
            "eviction only possible for bucket expiring < current block height"
        );

        // test that eviction works after expiration time
        System::set_block_number(block_height_2);
        let bucket_1 = Anchor::determine_pre_commit_eviction_bucket(block_height_0);

        // before eviction, the pre-commit data findable
        let a = Anchor::get_pre_commit(anchor_id_0);
        assert_eq!(a.identity, 1);
        assert_eq!(a.signing_root, signing_root);

        //do check counts, evict, check counts again
        assert_eq!(Anchor::get_pre_commits_count_in_evict_bucket(bucket_1), 1);
        assert_ok!(Anchor::evict_pre_commits(Origin::signed(1), bucket_1));
        assert_eq!(Anchor::get_pre_commits_count_in_evict_bucket(bucket_1), 0);

        // after eviction, the pre-commit data not findable
        let a_evicted = Anchor::get_pre_commit(anchor_id_0);
        assert_eq!(a_evicted.identity, 0);
        assert_eq!(a_evicted.expiration_block, 0);

        let bucket_2 = Anchor::determine_pre_commit_eviction_bucket(block_height_1);
        assert_eq!(Anchor::get_pre_commits_count_in_evict_bucket(bucket_2), 2);
        assert_ok!(Anchor::evict_pre_commits(Origin::signed(1), bucket_2));
        assert_eq!(Anchor::get_pre_commits_count_in_evict_bucket(bucket_2), 0);
    });
}

#[test]
fn pre_commit_at_4799_and_then_evict_before_expire_and_collaborator_succeed_commit() {
    new_test_ext().execute_with(|| {
        let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
        let (doc_root, signing_root, proof) = Test::test_document_hashes();
        let start_block = 4799;
        // expected expiry block of pre-commit
        let expiration_block = start_block + Anchor::pre_commit_expiration_duration_blocks(); // i.e 4799 + 480

        System::set_block_number(start_block);
        // happy
        assert_ok!(Anchor::pre_commit(
            Origin::signed(1),
            anchor_id,
            signing_root
        ));

        let a = Anchor::get_pre_commit(anchor_id);
        assert_eq!(a.expiration_block, expiration_block);
        // the edge case bug we had - pre-commit eviction time is less than its expiry time
        assert_eq!(
            Anchor::determine_pre_commit_eviction_bucket(expiration_block) > a.expiration_block,
            true
        );

        // this should not evict the pre-commit before its expired
        System::set_block_number(Anchor::determine_pre_commit_eviction_bucket(start_block) + 1);
        assert_ok!(Anchor::evict_pre_commits(
            Origin::signed(1),
            Anchor::determine_pre_commit_eviction_bucket(start_block)
        ));

        // fails
        assert_err!(
            Anchor::commit(
                Origin::signed(2),
                pre_image,
                doc_root,
                proof,
                common::MS_PER_DAY + 1
            ),
            "Pre-commit owned by someone else"
        );
    });
}

#[test]
fn pre_commit_and_then_evict_larger_than_max_evict() {
    new_test_ext().execute_with(|| {
        let block_height_0 = 1;
        let block_height_1 =
            Anchor::determine_pre_commit_eviction_bucket(block_height_0) + block_height_0;
        let signing_root = <Test as system::Trait>::Hashing::hash_of(&0);

        System::set_block_number(block_height_0);
        for idx in 0..MAX_LOOP_IN_TX + 6 {
            assert_ok!(Anchor::pre_commit(
                Origin::signed(1),
                <Test as system::Trait>::Hashing::hash_of(&idx),
                signing_root
            ));
        }

        System::set_block_number(block_height_1);
        let bucket_1 = Anchor::determine_pre_commit_eviction_bucket(block_height_0);

        //do check counts, evict, check counts again
        assert_eq!(Anchor::get_pre_commits_count_in_evict_bucket(bucket_1), 506);
        assert_ok!(Anchor::evict_pre_commits(Origin::signed(1), bucket_1));
        assert_eq!(Anchor::get_pre_commits_count_in_evict_bucket(bucket_1), 6);

        // evict again, now should be empty
        System::set_block_number(block_height_1 + 1);
        assert_ok!(Anchor::evict_pre_commits(Origin::signed(1), bucket_1));
        assert_eq!(Anchor::get_pre_commits_count_in_evict_bucket(bucket_1), 0);
    });
}
// #### End Pre Commit Eviction Tests

#[test]
fn anchor_evict_single_anchor_per_day_1000_days() {
    new_test_ext().execute_with(|| {
        let day = |n| common::MS_PER_DAY * n + 1;
        let (doc_root, signing_root, proof) = Test::test_document_hashes();
        let mut anchors = vec![];
        let verify_anchor_eviction = |day: usize, anchors: &Vec<H256>| {
            assert!(Anchor::get_anchor_by_id(anchors[day - 2]).is_none());
            assert_eq!(Anchor::get_latest_evicted_anchor_index(), (day - 1) as u64);
            assert_eq!(
                Anchor::get_anchor_id_by_index((day - 1) as u64),
                H256([0; 32])
            );
            assert!(Anchor::get_evicted_anchor_root_by_day((day - 1) as u32) != [0; 32]);
            assert_eq!(Anchor::get_anchor_evict_date(anchors[day - 2]), 0);
        };
        let verify_next_anchor_after_eviction = |day: usize, anchors: &Vec<H256>| {
            assert!(Anchor::get_anchor_by_id(anchors[day - 1]).is_some());
            assert_eq!(Anchor::get_anchor_id_by_index(day as u64), anchors[day - 1]);
            assert_eq!(
                Anchor::get_anchor_evict_date(anchors[day - 1]),
                (day + 1) as u32
            );
        };

        // create 1000 anchors one per day
        for i in 0..1000 {
            let random_seed = <randomness_collective_flip::Module<Test>>::random_seed();
            let pre_image = (random_seed, i).using_encoded(<Test as system::Trait>::Hashing::hash);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);

            assert_ok!(Anchor::commit(
                Origin::signed(1),
                pre_image,
                doc_root,
                proof,
                day(i + 1)
            ));

            assert!(Anchor::get_anchor_by_id(anchor_id).is_some());
            assert_eq!(Anchor::get_latest_anchor_index(), i + 1);
            assert_eq!(Anchor::get_anchor_id_by_index(i + 1), anchor_id);
            assert_eq!(Anchor::get_latest_evicted_anchor_index(), 0);
            assert_eq!(Anchor::get_anchor_evict_date(anchor_id), (i + 2) as u32);

            anchors.push(anchor_id);
        }

        // eviction on day 3
        <timestamp::Module<Test>>::set_timestamp(day(2));
        assert!(Anchor::get_anchor_by_id(anchors[0]).is_some());
        assert_ok!(Anchor::evict_anchors(Origin::signed(1)));
        verify_anchor_eviction(2, &anchors);
        assert_eq!(
            Anchor::get_evicted_anchor_root_by_day(2),
            [
                159, 183, 44, 117, 34, 66, 8, 221, 84, 27, 226, 237, 170, 17, 75, 57, 171, 140, 65,
                234, 14, 217, 51, 245, 38, 19, 101, 199, 23, 210, 58, 163
            ]
        );

        verify_next_anchor_after_eviction(2, &anchors);

        // do the same as above for next 99 days without child trie root verification
        for i in 3..102 {
            <timestamp::Module<Test>>::set_timestamp(day(i as u64));
            assert!(Anchor::get_anchor_by_id(anchors[i - 2]).is_some());

            // evict
            assert_ok!(Anchor::evict_anchors(Origin::signed(1)));
            verify_anchor_eviction(i, &anchors);
            verify_next_anchor_after_eviction(i, &anchors);
        }
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 100);

        // test out limit on the number of anchors removed at a time
        // eviction on day 602, i.e 501 anchors to be removed one anchor
        // per day from the last eviction on day 102
        <timestamp::Module<Test>>::set_timestamp(day(602));
        assert!(Anchor::get_anchor_by_id(anchors[600]).is_some());
        // evict
        assert_ok!(Anchor::evict_anchors(Origin::signed(1)));
        // verify anchor data has been removed until 520th anchor
        for i in 102..602 {
            assert!(Anchor::get_anchor_by_id(anchors[i - 2]).is_none());
            assert!(Anchor::get_evicted_anchor_root_by_day(i as u32) != [0; 32]);
        }

        assert!(Anchor::get_anchor_by_id(anchors[600]).is_none());
        assert!(Anchor::get_anchor_by_id(anchors[601]).is_some());

        // verify that 601st anchors` indexes are left still because of 500 limit while
        // 600th anchors` indexes have been removed
        // 600th anchor
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 600);
        assert_eq!(Anchor::get_anchor_id_by_index(600), H256([0; 32]));
        assert_eq!(Anchor::get_anchor_evict_date(anchors[599]), 0);
        // 601st anchor indexes are left
        assert!(Anchor::get_anchor_id_by_index(601) != H256([0; 32]));
        assert_eq!(Anchor::get_anchor_evict_date(anchors[600]), 602);

        // call evict on same day to remove the remaining indexes
        assert_ok!(Anchor::evict_anchors(Origin::signed(1)));
        // verify that 521st anchors indexes are removed since we called a second time
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 601);
        assert_eq!(Anchor::get_anchor_id_by_index(601), H256([0; 32]));
        assert_eq!(Anchor::get_anchor_evict_date(anchors[600]), 0);

        // remove remaining anchors
        <timestamp::Module<Test>>::set_timestamp(day(1001));
        assert!(Anchor::get_anchor_by_id(anchors[999]).is_some());
        assert_ok!(Anchor::evict_anchors(Origin::signed(1)));
        assert!(Anchor::get_anchor_by_id(anchors[999]).is_none());
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 1000);
        assert_eq!(Anchor::get_anchor_id_by_index(1000), H256([0; 32]));
        assert_eq!(Anchor::get_anchor_evict_date(anchors[999]), 0);
    });
}

#[test]
fn test_remove_anchor_indexes() {
    new_test_ext().execute_with(|| {
        let day = |n| common::MS_PER_DAY * n + 1;
        let (doc_root, signing_root, proof) = Test::test_document_hashes();

        // create 2000 anchors that expire on same day
        for i in 0..2000 {
            let random_seed = <randomness_collective_flip::Module<Test>>::random_seed();
            let pre_image = (random_seed, i).using_encoded(<Test as system::Trait>::Hashing::hash);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            assert_ok!(Anchor::commit(
                Origin::signed(1),
                pre_image,
                doc_root,
                proof,
                // all anchors expire on same day
                day(1)
            ));
        }
        assert_eq!(Anchor::get_latest_anchor_index(), 2000);

        // first MAX_LOOP_IN_TX items
        let removed = Anchor::remove_anchor_indexes(2);
        assert_eq!(removed as u64, MAX_LOOP_IN_TX);
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 500);

        // second MAX_LOOP_IN_TX items
        let removed = Anchor::remove_anchor_indexes(2);
        assert_eq!(removed as u64, MAX_LOOP_IN_TX);
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 1000);

        // third MAX_LOOP_IN_TX items
        let removed = Anchor::remove_anchor_indexes(2);
        assert_eq!(removed as u64, MAX_LOOP_IN_TX);
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 1500);

        // fourth MAX_LOOP_IN_TX items
        let removed = Anchor::remove_anchor_indexes(2);
        assert_eq!(removed as u64, MAX_LOOP_IN_TX);
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 2000);

        // all done
        let removed = Anchor::remove_anchor_indexes(2);
        assert_eq!(removed, 0);
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 2000);
    });
}

#[test]
fn test_same_day_1001_anchors() {
    new_test_ext().execute_with(|| {
        let day = |n| common::MS_PER_DAY * n + 1;
        let (doc_root, signing_root, proof) = Test::test_document_hashes();
        let mut anchors = vec![];

        // create 1001 anchors that expire on same day
        for i in 0..1001 {
            let random_seed = <randomness_collective_flip::Module<Test>>::random_seed();
            let pre_image = (random_seed, i).using_encoded(<Test as system::Trait>::Hashing::hash);
            let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
            assert_ok!(Anchor::commit(
                Origin::signed(1),
                pre_image,
                doc_root,
                proof,
                // all anchors expire on same day
                day(1)
            ));
            anchors.push(anchor_id);
        }
        assert_eq!(Anchor::get_latest_anchor_index(), 1001);

        // first 500
        <timestamp::Module<Test>>::set_timestamp(day(2));
        assert!(Anchor::get_anchor_by_id(anchors[999]).is_some());
        assert_ok!(Anchor::evict_anchors(Origin::signed(1)));
        assert!(Anchor::get_anchor_by_id(anchors[999]).is_none());
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 500);
        assert_eq!(Anchor::get_anchor_id_by_index(500), H256([0; 32]));
        assert_eq!(Anchor::get_anchor_evict_date(anchors[499]), 0);
        assert_eq!(
            Anchor::get_evicted_anchor_root_by_day(2),
            [
                50, 46, 7, 230, 27, 31, 182, 47, 154, 182, 204, 174, 29, 71, 116, 110, 187, 42,
                101, 13, 79, 220, 149, 142, 34, 4, 93, 112, 209, 17, 24, 167
            ]
        );

        // second 500
        assert_ok!(Anchor::evict_anchors(Origin::signed(1)));
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 1000);
        assert_eq!(Anchor::get_anchor_id_by_index(1000), H256([0; 32]));
        assert_eq!(Anchor::get_anchor_evict_date(anchors[999]), 0);

        // remaining
        assert_ok!(Anchor::evict_anchors(Origin::signed(1)));
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 1001);
        assert_eq!(Anchor::get_anchor_id_by_index(1001), H256([0; 32]));
        assert_eq!(Anchor::get_anchor_evict_date(anchors[1000]), 0);

        // all done
        assert_ok!(Anchor::evict_anchors(Origin::signed(1)));
        assert_eq!(Anchor::get_latest_evicted_anchor_index(), 1001);
    });
}

#[test]
#[ignore]
fn basic_commit_perf() {
    new_test_ext().execute_with(|| {
        let mut elapsed: u128 = 0;
        for i in 0..100000 {
            let random_seed = <randomness_collective_flip::Module<Test>>::random_seed();
            let pre_image = (random_seed, i).using_encoded(<Test as system::Trait>::Hashing::hash);
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
                proof,
                1
            ));

            elapsed = elapsed + now.elapsed().as_micros();
        }

        println!("time {}", elapsed);
    });
}

#[test]
fn test_move_anchor_without_origin() {
    new_test_ext().execute_with(|| {
        let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
        assert_err!(
            Anchor::move_anchor(Origin::NONE, anchor_id),
            "RequireSignedOrigin"
        );
    });
}

#[test]
fn test_move_anchor_missing_anchor() {
    new_test_ext().execute_with(|| {
        let anchor_id = <Test as system::Trait>::Hashing::hash_of(&0);
        assert_err!(
            Anchor::move_anchor(Origin::signed(0), anchor_id),
            "Anchor doesn't exist"
        );
    });
}

#[test]
fn test_move_anchor_success() {
    new_test_ext().execute_with(|| {
        let pre_image = <Test as system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as system::Trait>::Hashing::hash);
        // commit anchor
        assert_ok!(Anchor::commit(
            Origin::signed(2),
            pre_image,
            <Test as system::Trait>::Hashing::hash_of(&0),
            <Test as system::Trait>::Hashing::hash_of(&0),
            common::MS_PER_DAY + 1
        ));

        assert_ok!(Anchor::move_anchor(Origin::signed(0), anchor_id));
    });
}
