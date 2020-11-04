use sp_core::{Encode, U256};
use sp_runtime::traits::{Hash, SaturatedConversion};
use frame_system::{ensure_none, ensure_root, ensure_signed};
use crate::constants::currency;
use sp_std::{vec::Vec, convert::TryInto};
use frame_support::{decl_module, decl_storage, decl_event, decl_error,
                    traits::{Get, EnsureOrigin, Currency, ExistenceRequirement::AllowDeath},
                    ensure, dispatch::DispatchResult};
use sp_runtime::{
    ModuleId,
    traits::{AccountIdConversion, CheckedSub},
    transaction_validity::{
        TransactionValidity, ValidTransaction, InvalidTransaction, TransactionSource,
        TransactionPriority,
    }
};

const MODULE_ID: ModuleId = ModuleId(*b"ct/claim");
const MIN_PAYOUT: node_primitives::Balance = 5 * currency::RAD;

pub trait Trait: frame_system::Trait + pallet_balances::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

    /// An expected duration of the session.
    ///
    /// This parameter is used to determine the longevity of `heartbeat` transaction
    /// and a rough time when we should start considering sending heartbeats,
    /// since the workers avoids sending them at the very beginning of the session, assuming
    /// there is a chance the authority will produce a block and they won't be necessary.
    type SessionDuration: Get<Self::BlockNumber>;

    /// A configuration for base priority of unsigned transactions.
    ///
    /// This is exposed so that it can be tuned for particular runtime, when
    /// multiple pallets send unsigned transactions.
    type UnsignedPriority: Get<TransactionPriority>;

    type AdminOrigin: EnsureOrigin<Self::Origin>;

    type Currency: Currency<Self::AccountId>;
}

decl_storage! {
    trait Store for Module<T: Trait> as RadClaims {
        /// Total unclaimed rewards for an account.
        AccountBalances get(fn get_account_balance): map hasher(blake2_128_concat) T::AccountId => T::Balance = 0.into();
        /// Map of root hashes that correspond to lists of RAD reward claim amounts per account.
        RootHashes get(fn get_root_hash): map hasher(blake2_128_concat) T::Hash => bool;
        /// Account that is allowed to upload new root hashes.
        UploadAccount get(fn get_upload_account): T::AccountId;
    }
}

decl_error! {
    pub enum Error for Module<T: Trait>{
        /// The combination of account id, amount, and proofs vector in a claim was invalid.
        InvalidProofs,
        /// The payout amount attempting to be claimed is less than the minimum allowed by [MIN_PAYOUT].
        UnderMinPayout,
        /// Amount being claimed is less than the available amount in [AccountBalances].
        InsufficientBalance,
        /// Protected operation, must be performed by admin
        MustBeAdmin,
    }
}

decl_event! {
    pub enum Event<T> where
        <T as frame_system::Trait>::AccountId,
        <T as frame_system::Trait>::Hash,
        <T as pallet_balances::Trait>::Balance,
    {
        Claimed(AccountId, Balance),
        RootHashStored(Hash),
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        /// Claims RAD tokens awarded through tinlake investments
        /// Feeless dispatchable function
        /// The extrinsic is validated by the custom `validate_unsigned` function below
        ///
        /// # <weight>
        /// - Based on hashes length
        /// # </weight>
        #[weight = 195_000_000]
        pub fn claim(origin,
                     account_id: T::AccountId,
                     amount: T::Balance,
                     sorted_hashes: Vec<T::Hash>,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let claimed = Self::get_account_balance(&account_id);

            // Payout = amount - claim
            let payout = amount.checked_sub(&claimed)
                .ok_or(Error::<T>::InsufficientBalance)?;

            // Payout must not be less than minimum allowed
            ensure!(payout >= MIN_PAYOUT.saturated_into(),
                    Error::<T>::UnderMinPayout);

            let source = MODULE_ID.into_account();
            // Checking balance of Module account before proceeding
            let current_balance = T::Currency::free_balance(&source);
            ensure!(U256::from(current_balance.saturated_into()) >= U256::from(payout.saturated_into()), Error::<T>::InsufficientBalance);

            // Set account balance to amount
            AccountBalances::<T>::insert(account_id.clone(), amount);

            // Transfer payout amount
            <pallet_balances::Module<T> as Currency<_>>::transfer(
                &source,
                &account_id,
                payout,
                AllowDeath,
            )?;

            Self::deposit_event(RawEvent::Claimed(account_id, amount));

            Ok(())
        }

        /// Admin function that sets the allowed upload account to add root hashes
        /// Controlled by custom origin or root
        /// 
        /// # <weight>
        /// - Based on origin check and write op
        /// # </weight>
        #[weight = 190_000_000]
        pub fn set_upload_account(origin, account_id: T::AccountId) -> DispatchResult {
            Self::can_update_upload_account(origin)?;

            <UploadAccount<T>>::put(account_id);

            Ok(())
        }

        /// Stores root hash for correspondent claim merkle tree run
        ///
        /// # <weight>
        /// - Based on origin check and write op
        /// # </weight>
        #[weight = 185_000_000]
        pub fn store_root_hash(origin, root_hash: T::Hash) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(Self::get_upload_account() == who, Error::<T>::MustBeAdmin);
            <RootHashes<T>>::insert(root_hash, true);
            Self::deposit_event(RawEvent::RootHashStored(root_hash));

            Ok(())
        }

    }
}

impl<T: Trait> Module<T> {
    /// Hash a:b if a < b, else b:a. Uses the runtime module's hasher.
    pub fn sorted_hash_of(a: &T::Hash, b: &T::Hash) -> T::Hash {
        let mut h: Vec<u8> = Vec::with_capacity(64);
        if a < b {
            h.extend_from_slice(a.as_ref());
            h.extend_from_slice(b.as_ref());
        } else {
            h.extend_from_slice(b.as_ref());
            h.extend_from_slice(a.as_ref());
        }

        T::Hashing::hash_of(&h).into()
    }

    /// Returns true if the given origin can update the upload account
    fn can_update_upload_account(origin: T::Origin) -> DispatchResult {
        T::AdminOrigin::try_origin(origin)
            .map(|_| ())
            .or_else(ensure_root)?;

        Ok(())
    }
}

impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
    type Call = Call<T>;

    fn validate_unsigned(
        _source: TransactionSource,
        call: &Self::Call,
    ) -> TransactionValidity {
        if let Call::claim(account_id, amount, sorted_hashes) = call {
            // Number of proofs should practically never be >30. Checking this
            // blocks abuse.
            if sorted_hashes.len() > 30 {
                return InvalidTransaction::BadProof.into()
            }

            // Concat account id : amount
            let mut v: Vec<u8> = account_id.encode();
            v.extend(amount.encode());

            // Generate root hash
            let leaf_hash = T::Hashing::hash_of(&v);
            let mut root_hash = sorted_hashes.iter()
                .fold(leaf_hash, |acc, hash| Self::sorted_hash_of(&acc, hash));

            // Initial runs might only have trees of single leaves,
            // in this case leaf_hash is as well root_hash
            if sorted_hashes.len() == 0 {
                root_hash = leaf_hash;
            }

            // Check that root exists in root hash storage
            if Self::get_root_hash(root_hash) == true {
                return ValidTransaction::with_tag_prefix("RADclaim")
                    .priority(T::UnsignedPriority::get())
                    .longevity(TryInto::<u64>::try_into(
                        T::SessionDuration::get() / 2.into())
                        .unwrap_or(64_u64))
                    .propagate(true)
                    .build()
            } else {
                return InvalidTransaction::BadProof.into();
            }
        }

        InvalidTransaction::Call.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use frame_support::{
        assert_err, assert_ok, impl_outer_origin,
        ord_parameter_types, parameter_types, weights::Weight,
    };
    use frame_system::EnsureSignedBy;
    use sp_core::H256;
    use sp_runtime::Perbill;
    use sp_runtime::{
        testing::Header,
        traits::{BadOrigin, BlakeTwo256, Hash, IdentityLookup},
    };
    pub use pallet_balances as balances;

    impl_outer_origin! {
        pub enum Origin for Test  where system = frame_system {}
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
        type AccountData = balances::AccountData<u128>;
        type OnNewAccount = ();
        type OnKilledAccount = balances::Module<Test>;
        type DbWeight = ();
        type BlockExecutionWeight = ();
        type ExtrinsicBaseWeight = ();
        type MaximumExtrinsicWeight = ();
        type BaseCallFilter = ();
        type SystemWeightInfo = ();
        type MigrateAccount = ();
    }
    ord_parameter_types! {
        pub const One: u64 = 1;
        pub const SessionDuration: u64 = 10 as u64;
        pub const UnsignedPriority: TransactionPriority = TransactionPriority::max_value();
    }

    impl Trait for Test {
        type Event = ();
        type SessionDuration = SessionDuration;
        type UnsignedPriority = UnsignedPriority;
        type AdminOrigin = EnsureSignedBy<One, u64>;
        type Currency = Balances;
    }

    parameter_types! {
        pub const ExistentialDeposit: u64 = 1;
    }
    impl pallet_balances::Trait for Test {
        type Balance = u128;
        type DustRemoval = ();
        type Event = ();
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
    }

    type RadClaims = Module<Test>;
    type System = frame_system::Module<Test>;
    type Balances = pallet_balances::Module<Test>;

    pub const ADMIN: u64 = 0x1;
    pub const USER_A: u64 = 0x2;
    // USER_B does not have existential balance
    pub const USER_B: u64 = 0x3;
    pub const ENDOWED_BALANCE: u128 = 10000 * currency::RAD;

    // This function basically just builds a genesis storage key/value store according to
    // our desired mockup.
    fn new_test_ext() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        let claims_module_id = MODULE_ID.into_account();
        // pre-fill balances
        pallet_balances::GenesisConfig::<Test> {
            balances: vec![(ADMIN, ENDOWED_BALANCE), (USER_A, 1), (claims_module_id, ENDOWED_BALANCE)],
        }
            .assimilate_storage(&mut t)
            .unwrap();
        t.into()
    }

    #[test]
    fn can_upload_account() {
        new_test_ext().execute_with(|| {
            assert_err!(RadClaims::can_update_upload_account(Origin::signed(USER_A)), BadOrigin);
            assert_ok!(RadClaims::can_update_upload_account(Origin::signed(ADMIN)));
        });
    }

    #[test]
    fn set_upload_account() {
        new_test_ext().execute_with(|| {
            assert_eq!(RadClaims::get_upload_account(), 0x0);
            assert_err!(RadClaims::set_upload_account(Origin::signed(USER_A), USER_A), BadOrigin);
            assert_ok!(RadClaims::set_upload_account(Origin::signed(ADMIN), USER_A));
            assert_eq!(RadClaims::get_upload_account(), USER_A);
        });
    }

    #[test]
    fn store_root_hash() {
        new_test_ext().execute_with(|| {
            assert_eq!(RadClaims::get_upload_account(), 0x0);
            // USER_A not allowed to upload hash
            let pre_image = <Test as frame_system::Trait>::Hashing::hash_of(&1);
            assert_err!(
                RadClaims::store_root_hash(Origin::signed(USER_A), pre_image),
                Error::<Test>::MustBeAdmin
            );
            // Adding ADMIN as allowed upload account
            assert_ok!(RadClaims::set_upload_account(Origin::signed(ADMIN), ADMIN));
            assert_eq!(RadClaims::get_upload_account(), ADMIN);
            assert_ok!(RadClaims::store_root_hash(Origin::signed(ADMIN), pre_image));
            assert_eq!(RadClaims::get_root_hash(pre_image), true);
        });
    }

    #[test]
    fn claim() {
        new_test_ext().execute_with(|| {
            // Random sorted hashes
            let sorted_hashes: [H256; 3] = [[0; 32].into(), [0; 32].into(), [0; 32].into()];

            // Bad origin, signed vs unsigned
            assert_err!(
                RadClaims::claim(Origin::signed(USER_B), USER_B, 100 * currency::RAD, sorted_hashes.to_vec()),
                BadOrigin
            );

            // Minimum payout not met
            assert_err!(
                RadClaims::claim(Origin::none(), USER_B, 4 * currency::RAD, sorted_hashes.to_vec()),
                Error::<Test>::UnderMinPayout
            );

            // Claims Module Account does not have enough balance
            assert_err!(
                RadClaims::claim(Origin::none(), USER_B, 10001 * currency::RAD, sorted_hashes.to_vec()),
                Error::<Test>::InsufficientBalance
            );

            // Ok
            let account_balance = <pallet_balances::Module<Test>>::free_balance(USER_B);
            assert_ok!(RadClaims::claim(Origin::none(), USER_B, 100 * currency::RAD, sorted_hashes.to_vec()));
            assert_eq!(RadClaims::get_account_balance(USER_B), 100 * currency::RAD);
            let account_new_balance = <pallet_balances::Module<Test>>::free_balance(USER_B);
            assert_eq!(account_new_balance, account_balance + 100 * currency::RAD);

            // Knowing that account has a balance of 100, trying to claim 50 will fail
            // Since balance logic is accumulative
            assert_err!(
                RadClaims::claim(Origin::none(), USER_B, 50 * currency::RAD, sorted_hashes.to_vec()),
                Error::<Test>::InsufficientBalance
            );

        });
    }

    #[test]
    fn validate_unsigned_check() {
        new_test_ext().execute_with(|| {
            let amount: u128 = 100 * currency::RAD;
            let sorted_hashes_long: [H256; 31] = [
                [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(),
                [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(),
                [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(),
                [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(),
                [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(), [0; 32].into(),
                [0; 32].into()
            ];

            // Abuse DDoS attach check
            let inner_long = Call::claim(USER_B, amount, sorted_hashes_long.to_vec());
            assert_err!(
                <RadClaims as sp_runtime::traits::ValidateUnsigned>::pre_dispatch(&inner_long),
                InvalidTransaction::BadProof
            );

            // Root hash was never stored beforehand
            let one_sorted_hashes: [H256; 1] = [[0; 32].into()];
            let inner = Call::claim(USER_B, amount, one_sorted_hashes.to_vec());
            assert_err!(
                <RadClaims as sp_runtime::traits::ValidateUnsigned>::pre_dispatch(&inner),
                InvalidTransaction::BadProof
            );

            assert_ok!(RadClaims::set_upload_account(Origin::signed(ADMIN), ADMIN));
            let mut v: Vec<u8> = USER_B.encode();
            v.extend(amount.encode());

            // Single-leaf tree
            let inner_single = Call::claim(USER_B, amount, [].to_vec());
            let leaf_hash = <Test as frame_system::Trait>::Hashing::hash_of(&v);
            assert_ok!(RadClaims::store_root_hash(Origin::signed(ADMIN), leaf_hash));
            assert_ok!(<RadClaims as sp_runtime::traits::ValidateUnsigned>::pre_dispatch(&inner_single));

            // Two-leaf tree
            let preimage = RadClaims::sorted_hash_of(&leaf_hash, &one_sorted_hashes[0]);
            assert_ok!(RadClaims::store_root_hash(Origin::signed(ADMIN), preimage));
            assert_ok!(<RadClaims as sp_runtime::traits::ValidateUnsigned>::pre_dispatch(&inner));

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
            let node_0 = RadClaims::sorted_hash_of(&leaf_hash_0, &leaf_hash_1);
            let node_1 = RadClaims::sorted_hash_of(&leaf_hash_2, &leaf_hash_3);
            let node_2 = RadClaims::sorted_hash_of(&leaf_hash_4, &leaf_hash_5);
            let node_3 = RadClaims::sorted_hash_of(&leaf_hash_6, &leaf_hash_7);
            let node_4 = RadClaims::sorted_hash_of(&leaf_hash_8, &leaf_hash_9);
            let node_00 = RadClaims::sorted_hash_of(&node_0, &node_1);
            let node_01 = RadClaims::sorted_hash_of(&node_2, &node_3);
            let node_000 = RadClaims::sorted_hash_of(&node_00, &node_01);
            let node_root = RadClaims::sorted_hash_of(&node_000, &node_4);

            let four_sorted_hashes: [H256; 4] = [leaf_hash_3.into(), node_0.into(), node_01.into(), node_4.into()];
            let inner_three = Call::claim(USER_B, amount, four_sorted_hashes.to_vec());
            assert_ok!(RadClaims::store_root_hash(Origin::signed(ADMIN), node_root));
            assert_ok!(<RadClaims as sp_runtime::traits::ValidateUnsigned>::pre_dispatch(&inner_three));
        });
    }
}