use crate::{anchor, proofs, proofs::Proof};
use frame_support::{
    decl_event, decl_module, dispatch::DispatchResult, ensure, weights::SimpleDispatchInfo,
};
use frame_system::{self as system, ensure_signed};
use sp_core::H256;
use sp_std::vec::Vec;

pub trait Trait: anchor::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_event!(
    pub enum Event<T> where <T as frame_system::Trait>::Hash {
        DepositAsset(Hash),
    }
);

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin  {
        fn deposit_event() = default;

        /// Validates the proofs provided against the document root associated with the anchor_id.
        /// Once the proofs are verified, we create a bundled hash (deposit_address + [proof[i].hash])
        /// Bundled Hash is deposited to an DepositAsset event for bridging purposes.
        ///
        /// # <weight>
        /// - depends on the arguments
        /// # </weight>
        #[weight = SimpleDispatchInfo::FixedNormal(1_500_000)]
        fn validate_mint(origin, anchor_id: T::Hash, deposit_address: [u8; 20], pfs: Vec<Proof>, static_proofs: [H256;3]) -> DispatchResult {
            ensure_signed(origin)?;

            // get the anchor data from anchor ID
            let anchor_data = <anchor::Module<T>>::get_anchor_by_id(anchor_id).ok_or("Anchor doesn't exist")?;

            // validate proofs
            ensure!(Self::validate_proofs(anchor_data.get_doc_root(), &pfs, static_proofs), "Invalid proofs");

            // get the bundled hash
            let bundled_hash = Self::get_bundled_hash(pfs, deposit_address);

            Self::deposit_event(RawEvent::DepositAsset(bundled_hash));

            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// Validates the proofs again the provided doc_root.
    /// returns false if any proofs are invalid.
    fn validate_proofs(doc_root: T::Hash, pfs: &Vec<Proof>, static_proofs: [H256; 3]) -> bool {
        proofs::validate_proofs(H256::from_slice(doc_root.as_ref()), pfs, static_proofs)
    }

    /// Returns a Keccak hash of deposit_address + hash(keccak(name+value+salt)) of each proof provided.
    fn get_bundled_hash(pfs: Vec<Proof>, deposit_address: [u8; 20]) -> T::Hash {
        let bh = proofs::bundled_hash(pfs, deposit_address);
        let mut res: T::Hash = Default::default();
        res.as_mut().copy_from_slice(&bh[..]);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::common;
    use crate::fees;
    use crate::proofs::Proof;
    use codec::Encode;
    use frame_support::{
        assert_err, assert_ok, impl_outer_origin, parameter_types, weights::Weight,
    };
    use sp_core::H256;
    use sp_runtime::{
        testing::Header,
        traits::{BadOrigin, BlakeTwo256, Hash, IdentityLookup},
        Perbill,
    };

    impl_outer_origin! {
        pub enum Origin for Test {}
    }

    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;

    type Nfts = super::Module<Test>;
    type Anchor = anchor::Module<Test>;

    parameter_types! {
        pub const BlockHashCount: u64 = 250;
        pub const MaximumBlockWeight: Weight = 1024;
        pub const MaximumBlockLength: u32 = 2 * 1024;
        pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    }

    impl frame_system::Trait for Test {
        type AccountId = u64;
        type Call = ();
        type Lookup = IdentityLookup<Self::AccountId>;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type Header = Header;
        type Event = ();
        type Origin = Origin;
        type BlockHashCount = BlockHashCount;
        type MaximumBlockWeight = MaximumBlockWeight;
        type MaximumBlockLength = MaximumBlockLength;
        type AvailableBlockRatio = AvailableBlockRatio;
        type Version = ();
        type ModuleToIndex = ();
    }

    impl anchor::Trait for Test {}

    impl Trait for Test {
        type Event = ();
    }

    impl pallet_timestamp::Trait for Test {
        type Moment = u64;
        type OnTimestampSet = ();
        type MinimumPeriod = ();
    }

    impl fees::Trait for Test {
        type Event = ();
        type FeeChangeOrigin = frame_system::EnsureRoot<u64>;
    }

    parameter_types! {
        pub const ExistentialDeposit: u64 = 0;
        pub const TransferFee: u64 = 0;
        pub const CreationFee: u64 = 0;
        pub const TransactionBaseFee: u64 = 0;
        pub const TransactionByteFee: u64 = 0;
    }

    impl pallet_balances::Trait for Test {
        type Balance = u64;
        type OnFreeBalanceZero = ();
        type OnNewAccount = ();
        type Event = ();

        type DustRemoval = ();
        type TransferPayment = ();
        type ExistentialDeposit = ExistentialDeposit;
        type TransferFee = TransferFee;
        type CreationFee = CreationFee;
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();
        fees::GenesisConfig::<Test> {
            initial_fees: vec![(
                // anchoring state rent fee per day
                H256::from(&[
                    17, 218, 109, 31, 118, 29, 223, 155, 219, 76, 157, 110, 83, 3, 235, 212, 31,
                    97, 133, 141, 10, 86, 71, 161, 167, 191, 224, 137, 191, 146, 27, 233,
                ]),
                // state rent 0 for tests
                0,
            )],
        }
        .assimilate_storage(&mut t)
        .unwrap();
        t.into()
    }

    fn get_invalid_proof() -> (Proof, H256, [H256; 3]) {
        let proof = Proof::new(
            [
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 20, 48, 97, 34, 3, 169, 157, 88, 159,
            ]
            .into(),
            vec![
                [
                    113, 229, 58, 22, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
                    22, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
                ]
                .into(),
                [
                    133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
                    92, 23, 170, 4, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
                ]
                .into(),
            ],
        );

        let doc_root: H256 = [
            48, 123, 58, 192, 8, 62, 20, 55, 99, 52, 37, 73, 174, 123, 214, 104, 37, 41, 189, 170,
            205, 80, 158, 136, 224, 128, 128, 89, 55, 240, 32, 234,
        ]
        .into();

        let static_proofs: [H256; 3] = [
            [
                25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175,
                70, 161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
            ]
            .into(),
            [
                61, 164, 199, 22, 164, 251, 58, 14, 67, 56, 242, 60, 86, 203, 128, 203, 138, 129,
                237, 7, 29, 7, 39, 58, 250, 42, 14, 53, 241, 108, 187, 74,
            ]
            .into(),
            [
                70, 124, 133, 120, 103, 45, 94, 174, 176, 18, 151, 243, 104, 120, 12, 54, 217, 189,
                59, 222, 109, 64, 136, 203, 56, 136, 159, 115, 96, 101, 2, 185,
            ]
            .into(),
        ];

        (proof, doc_root, static_proofs)
    }

    fn get_valid_proof() -> (Proof, sp_core::H256, [H256; 3]) {
        let proof = Proof::new(
            [
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
            ]
            .into(),
            vec![
                [
                    113, 229, 58, 223, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
                    223, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
                ]
                .into(),
                [
                    133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
                    92, 232, 170, 46, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
                ]
                .into(),
                [
                    197, 248, 165, 165, 247, 119, 114, 231, 95, 114, 94, 16, 66, 142, 230, 184, 78,
                    203, 73, 104, 24, 82, 134, 154, 180, 129, 71, 223, 72, 31, 230, 15,
                ]
                .into(),
                [
                    50, 5, 28, 219, 118, 141, 222, 221, 133, 174, 178, 212, 71, 94, 64, 44, 80,
                    218, 29, 92, 77, 40, 241, 16, 126, 48, 119, 31, 6, 147, 224, 5,
                ]
                .into(),
            ],
        );

        let doc_root: H256 = [
            48, 123, 58, 192, 8, 62, 20, 55, 99, 52, 37, 73, 174, 123, 214, 104, 37, 41, 189, 170,
            205, 80, 158, 136, 224, 128, 128, 89, 55, 240, 32, 234,
        ]
        .into();

        let static_proofs: [H256; 3] = [
            [
                25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175,
                70, 161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
            ]
            .into(),
            [
                61, 164, 199, 22, 164, 251, 58, 14, 67, 56, 242, 60, 86, 203, 128, 203, 138, 129,
                237, 7, 29, 7, 39, 58, 250, 42, 14, 53, 241, 108, 187, 74,
            ]
            .into(),
            [
                70, 124, 133, 120, 103, 45, 94, 174, 176, 18, 151, 243, 104, 120, 12, 54, 217, 189,
                59, 222, 109, 64, 136, 203, 56, 136, 159, 115, 96, 101, 2, 185,
            ]
            .into(),
        ];

        (proof, doc_root, static_proofs)
    }

    fn get_params() -> (sp_core::H256, [u8; 20], Vec<Proof>, [H256; 3]) {
        let anchor_id = <Test as frame_system::Trait>::Hashing::hash_of(&0);
        let deposit_address: [u8; 20] = [0; 20];
        let pfs: Vec<Proof> = vec![];
        let static_proofs: [H256; 3] = [[0; 32].into(), [0; 32].into(), [0; 32].into()];
        (anchor_id, deposit_address, pfs, static_proofs)
    }

    #[test]
    fn bad_origin() {
        new_test_ext().execute_with(|| {
            let (anchor_id, deposit_address, pfs, static_proofs) = get_params();
            assert_err!(
                Nfts::validate_mint(Origin::NONE, anchor_id, deposit_address, pfs, static_proofs),
                BadOrigin
            );
        })
    }

    #[test]
    fn missing_anchor() {
        new_test_ext().execute_with(|| {
            let (anchor_id, deposit_address, pfs, static_proofs) = get_params();
            assert_err!(
                Nfts::validate_mint(
                    Origin::signed(1),
                    anchor_id,
                    deposit_address,
                    pfs,
                    static_proofs
                ),
                "Anchor doesn't exist"
            );
        })
    }

    #[test]
    fn invalid_proof() {
        new_test_ext().execute_with(|| {
            let deposit_address: [u8; 20] = [0; 20];
            let pre_image = <Test as frame_system::Trait>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<Test as frame_system::Trait>::Hashing::hash);
            let (pf, doc_root, static_proofs) = get_invalid_proof();
            assert_ok!(Anchor::commit(
                Origin::signed(2),
                pre_image,
                doc_root,
                <Test as frame_system::Trait>::Hashing::hash_of(&0),
                common::MS_PER_DAY + 1
            ));

            assert_err!(
                Nfts::validate_mint(
                    Origin::signed(1),
                    anchor_id,
                    deposit_address,
                    vec![pf],
                    static_proofs
                ),
                "Invalid proofs"
            );
        })
    }

    #[test]
    fn valid_proof() {
        new_test_ext().execute_with(|| {
            let deposit_address: [u8; 20] = [0; 20];
            let pre_image = <Test as frame_system::Trait>::Hashing::hash_of(&0);
            let anchor_id = (pre_image).using_encoded(<Test as frame_system::Trait>::Hashing::hash);
            let (pf, doc_root, static_proofs) = get_valid_proof();
            assert_ok!(Anchor::commit(
                Origin::signed(2),
                pre_image,
                doc_root,
                <Test as frame_system::Trait>::Hashing::hash_of(&0),
                common::MS_PER_DAY + 1
            ));

            assert_ok!(Nfts::validate_mint(
                Origin::signed(1),
                anchor_id,
                deposit_address,
                vec![pf],
                static_proofs
            ),);
        })
    }
}
