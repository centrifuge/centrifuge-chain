/// Handling state rent fee payments for specific transactions
use codec::{Decode, Encode};
use frame_support::{
    decl_event, decl_module, decl_storage,
    dispatch::DispatchResult,
    ensure,
    traits::{Currency, ExistenceRequirement, WithdrawReason},
    weights::SimpleDispatchInfo,
};
use frame_system::{self as system, ensure_root};
use sp_runtime::traits::EnsureOrigin;

/// The module's configuration trait.
pub trait Trait: frame_system::Trait + pallet_balances::Trait + pallet_authorship::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// Required origin for changing fees
    type FeeChangeOrigin: EnsureOrigin<Self::Origin>;
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Fee<Hash, Balance> {
    key: Hash,
    price: Balance,
}

decl_storage! {
    trait Store for Module<T: Trait> as Fees {
        Fees get(fee) : map hasher(blake2_256) T::Hash => Fee<T::Hash, T::Balance>;

        Version: u64;
    }
    add_extra_genesis {
        // Anchoring state rent fee per day
        config(initial_fees): Vec<(T::Hash, T::Balance)>;
        build(|config| Module::<T>::initialize_fees(&config.initial_fees))
    }
}

decl_event!(
    pub enum Event<T> where <T as frame_system::Trait>::Hash, <T as pallet_balances::Trait>::Balance {
        FeeChanged(Hash, Balance),
    }
);

decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        // Initializing events
        // this is needed only if you are using events in your module
        fn deposit_event() = default;

        /// Set the given fee for the key
        ///
        /// # <weight>
        /// - Independent of the arguments.
        /// - Contains a limited number of reads and writes.
        /// # </weight>
        #[weight = SimpleDispatchInfo::FixedOperational(1_000_000)]
        pub fn set_fee(origin, key: T::Hash, new_price: T::Balance) -> DispatchResult {
            Self::can_change_fee(origin)?;
            Self::change_fee(key, new_price);

            Self::deposit_event(RawEvent::FeeChanged(key, new_price));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// Called by any other module who wants to trigger a fee payment for a given account.
    /// The current fee price can be retrieved via Fees::price_of()
    pub fn pay_fee(from: T::AccountId, key: T::Hash) -> DispatchResult {
        ensure!(<Fees<T>>::contains_key(key), "fee not found for key");

        let single_fee = <Fees<T>>::get(key);
        Self::pay_fee_to_author(from, single_fee.price)?;

        Ok(())
    }

    /// Pay the given fee
    pub fn pay_fee_to_author(from: T::AccountId, fee: T::Balance) -> DispatchResult {
        let value = <pallet_balances::Module<T> as Currency<_>>::withdraw(
            &from,
            fee,
            WithdrawReason::Fee.into(),
            ExistenceRequirement::KeepAlive,
        )?;

        let author = <pallet_authorship::Module<T>>::author();
        <pallet_balances::Module<T> as Currency<_>>::resolve_creating(&author, value);
        Ok(())
    }

    /// Returns the current fee for the key
    pub fn price_of(key: T::Hash) -> Option<T::Balance> {
        //why this has been hashed again after passing to the function? sp_io::print(key.as_ref());
        if <Fees<T>>::contains_key(&key) {
            let single_fee = <Fees<T>>::get(&key);
            Some(single_fee.price)
        } else {
            None
        }
    }

    /// Returns true if the given origin can change the fee
    fn can_change_fee(origin: T::Origin) -> DispatchResult {
        T::FeeChangeOrigin::try_origin(origin)
            .map(|_| ())
            .or_else(ensure_root)?;

        Ok(())
    }

    /// Initialise fees for a fixed set of keys. i.e. For use in genesis
    fn initialize_fees(fees: &[(T::Hash, T::Balance)]) {
        fees.iter()
            .map(|(ref key, ref fee)| Self::change_fee(*key, *fee))
            .count();
    }

    /// Change the fee for the given key
    fn change_fee(key: T::Hash, fee: T::Balance) {
        let new_fee = Fee {
            key: key.clone(),
            price: fee,
        };
        <Fees<T>>::insert(key, new_fee);
    }
}

/// tests for fees module
#[cfg(test)]
mod tests {
    use super::*;

    use frame_support::{
        assert_err, assert_noop, assert_ok, dispatch::DispatchError, impl_outer_origin,
        ord_parameter_types, parameter_types, traits::FindAuthor, weights::Weight,
        ConsensusEngineId,
    };
    use frame_system::EnsureSignedBy;
    use sp_core::H256;
    use sp_runtime::Perbill;
    use sp_runtime::{
        testing::Header,
        traits::{BadOrigin, BlakeTwo256, Hash, IdentityLookup},
    };

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
        type AccountData = pallet_balances::AccountData<u64>;
        type OnNewAccount = ();
        type OnKilledAccount = pallet_balances::Module<Test>;
    }
    ord_parameter_types! {
        pub const One: u64 = 1;
    }
    impl Trait for Test {
        type Event = ();
        type FeeChangeOrigin = EnsureSignedBy<One, u64>;
    }
    parameter_types! {
        pub const ExistentialDeposit: u64 = 1;
    }
    impl pallet_balances::Trait for Test {
        type Balance = u64;
        type DustRemoval = ();
        type Event = ();
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
    }

    impl pallet_authorship::Trait for Test {
        type FindAuthor = AuthorGiven;
        type UncleGenerations = ();
        type FilterUncle = ();
        type EventHandler = ();
    }

    pub struct AuthorGiven;

    impl FindAuthor<u64> for AuthorGiven {
        fn find_author<'a, I>(_digests: I) -> Option<u64>
        where
            I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
        {
            Some(100)
        }
    }

    type Fees = Module<Test>;
    type System = frame_system::Module<Test>;

    // This function basically just builds a genesis storage key/value store according to
    // our desired mockup.
    fn new_test_ext() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        // pre-fill balances
        // 100 is the block author
        pallet_balances::GenesisConfig::<Test> {
            balances: vec![(1, 100000), (2, 100000), (100, 100)],
        }
        .assimilate_storage(&mut t)
        .unwrap();
        t.into()
    }

    #[test]
    fn can_change_fee() {
        new_test_ext().execute_with(|| {
            assert_noop!(Fees::can_change_fee(Origin::signed(2)), BadOrigin);
            assert_ok!(Fees::can_change_fee(Origin::signed(1)));
        });
    }

    #[test]
    fn multiple_new_fees_are_setable() {
        new_test_ext().execute_with(|| {
            let fee_key1 = <Test as frame_system::Trait>::Hashing::hash_of(&11111);
            let fee_key2 = <Test as frame_system::Trait>::Hashing::hash_of(&22222);

            let price1: <Test as pallet_balances::Trait>::Balance = 666;
            let price2: <Test as pallet_balances::Trait>::Balance = 777;

            assert_noop!(
                Fees::set_fee(Origin::signed(2), fee_key1, price1),
                BadOrigin
            );
            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key1, price1));
            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key2, price2));

            let loaded_fee1 = Fees::fee(fee_key1);
            assert_eq!(loaded_fee1.price, price1);

            let loaded_fee2 = Fees::fee(fee_key2);
            assert_eq!(loaded_fee2.price, price2);
        });
    }

    #[test]
    fn fee_is_re_setable() {
        new_test_ext().execute_with(|| {
            let fee_key = <Test as frame_system::Trait>::Hashing::hash_of(&11111);

            let initial_price: <Test as pallet_balances::Trait>::Balance = 666;
            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, initial_price));

            let loaded_fee = Fees::fee(fee_key);
            assert_eq!(loaded_fee.price, initial_price);

            let new_price: <Test as pallet_balances::Trait>::Balance = 777;
            assert_noop!(
                Fees::set_fee(Origin::signed(2), fee_key, new_price),
                BadOrigin
            );
            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, new_price));
            let again_loaded_fee = Fees::fee(fee_key);
            assert_eq!(again_loaded_fee.price, new_price);
        });
    }

    #[test]
    fn fee_payment_errors_if_not_set() {
        new_test_ext().execute_with(|| {
            let fee_key = <Test as frame_system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as pallet_balances::Trait>::Balance = 90000;
            let author_old_balance = <pallet_balances::Module<Test>>::total_balance(&100);

            assert_err!(Fees::pay_fee(1, fee_key), "fee not found for key");

            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));

            // initial time paying will succeed as sufficient balance + fee is set
            assert_ok!(Fees::pay_fee(1, fee_key));

            let author_new_balance = <pallet_balances::Module<Test>>::total_balance(&100);
            assert_eq!(author_new_balance - author_old_balance, fee_price);

            // second time paying will lead to account having insufficient balance
            assert_err!(
                Fees::pay_fee(1, fee_key),
                DispatchError::Module {
                    index: 0,
                    error: 3,
                    message: Some("InsufficientBalance"),
                }
            );
        });
    }

    #[test]
    fn fee_payment_errors_if_insufficient_balance() {
        new_test_ext().execute_with(|| {
            let fee_key = <Test as frame_system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as pallet_balances::Trait>::Balance = 90000;

            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));

            // account 3 is not endowed in the test setup
            assert_err!(
                Fees::pay_fee(3, fee_key),
                DispatchError::Module {
                    index: 0,
                    error: 3,
                    message: Some("InsufficientBalance"),
                }
            );
        });
    }

    #[test]
    fn fee_payment_subtracts_fees_from_account() {
        new_test_ext().execute_with(|| {
            let fee_key = <Test as frame_system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as pallet_balances::Trait>::Balance = 90000;
            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));

            // account 1 is endowed in test setup
            // initial time paying will succeed as sufficient balance + fee is set
            assert_ok!(Fees::pay_fee(1, fee_key));

            //second time paying will lead to account having insufficient balance
            assert_err!(
                Fees::pay_fee(1, fee_key),
                DispatchError::Module {
                    index: 0,
                    error: 3,
                    message: Some("InsufficientBalance"),
                }
            );
        });
    }

    #[test]
    fn fee_is_gettable() {
        new_test_ext().execute_with(|| {
            let fee_key = <Test as frame_system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as pallet_balances::Trait>::Balance = 90000;

            //First run, the fee is not set yet and should return None
            match Fees::price_of(fee_key) {
                Some(_x) => assert!(false, "Should not have a fee set yet"),
                None => assert!(true),
            }

            //After setting the fee, the correct fee should be returned
            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));
            //First run, the fee is not set yet and should return None
            match Fees::price_of(fee_key) {
                Some(x) => assert_eq!(fee_price, x),
                None => assert!(false, "Fee should have been set"),
            }
        });
    }
}
