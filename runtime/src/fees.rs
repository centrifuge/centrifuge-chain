use codec::{Decode, Encode};
use sr_primitives::traits::Hash;
/// Handling fees payments for specific transactions
/// Initially being hard-coded, later coming from the governance module
use support::{
    decl_event, decl_module, decl_storage,
    dispatch::Result,
    ensure,
    traits::{Currency, ExistenceRequirement, WithdrawReason},
    StorageMap,
    StorageValue
};
use system::ensure_signed;

// TODO tie in governance
//use super::validatorset;

/// The module's configuration trait.
/// TODO tie in governance
//pub trait Trait: system::Trait + balances::Trait + validatorset::Trait{
pub trait Trait: system::Trait + balances::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Fee<Hash, Balance> {
    key: Hash,
    price: Balance,
}

decl_storage! {
    trait Store for Module<T: Trait> as Fees {
        Fees get(fee) : map T::Hash => Fee<T::Hash, T::Balance>;

        Version: u64;
    }
    add_extra_genesis {
		config(initial_fees): Vec<(T::Hash, T::Balance)>;
		build(|
			storage: &mut (sr_primitives::StorageOverlay, sr_primitives::ChildrenStorageOverlay),
			config: &GenesisConfig<T>
		| {
			runtime_io::with_storage(
				storage,
				|| Module::<T>::initialize_fees(&config.initial_fees),
			);
		})
	}
}

decl_event!(
	pub enum Event<T> where AccountId = <T as system::Trait>::AccountId, <T as system::Trait>::Hash, <T as balances::Trait>::Balance {
		FeeChanged(AccountId, Hash, Balance),
	}
);

decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        // Initializing events
        // this is needed only if you are using events in your module
        fn deposit_event<T>() = default;

        fn on_initialize(_now: T::BlockNumber) {
            if <Version>::get() == 0 {
                // do first upgrade
                // ...

                // uncomment when upgraded
                // <Version<T>>::put(1);
            }
        }

        /// Set the given fee for the key
        pub fn set_fee(origin, key: T::Hash, new_price: T::Balance) -> Result {
            let sender = ensure_signed(origin)?;
            Self::can_change_fee(sender.clone())?;
            Self::change_fee(key, new_price);

            Self::deposit_event(RawEvent::FeeChanged(sender, key, new_price));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {

    /// Called by any other module who wants to trigger a fee payment
    /// for a given account.
    /// The current fee price can be retrieved via Fees::price_of()
    pub fn pay_fee(who: T::AccountId, key: T::Hash) -> Result {
        ensure!(<Fees<T>>::exists(key), "fee not found for key");

        let single_fee = <Fees<T>>::get(key);
        Self::pay_fee_given(who, single_fee.price)?;

        Ok(())
    }

    /// Pay the given fee
    pub fn pay_fee_given(who: T::AccountId, fee: T::Balance) -> Result {
        let _ = <balances::Module<T> as Currency<_>>::withdraw(
            &who,
            fee,
            WithdrawReason::Fee,
            ExistenceRequirement::KeepAlive,
        )?;
        Ok(())
    }

    pub fn price_of(key: T::Hash) -> Option<T::Balance> {
        //why this has been hashed again after passing to the function? runtime_io::print(key.as_ref());
        if <Fees<T>>::exists(&key) {
            let single_fee = <Fees<T>>::get(&key);
            Some(single_fee.price)
        } else {
            None
        }
    }

    fn can_change_fee(_who: T::AccountId) -> Result {
        //TODO add auth who can change fees
        //        ensure!(<validatorset::Module<T>>::is_validator(who), "Not authorized to change fees.");
        Ok(())
    }

    /// Initialise fees for a fixed set of keys. i.e. For use in genesis
    fn initialize_fees(fees: &[(T::Hash, T::Balance)]) {
        fees.iter()
            .map(|(ref key, ref fee)|
                Self::change_fee(*key, *fee))
            .count();
    }

    /// change the fee for the given key
    fn change_fee(key: T::Hash, fee: T::Balance) {
        let new_fee = Fee{
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

    use runtime_io::with_externalities;
    use primitives::{H256, Blake2Hasher};
    use support::{impl_outer_origin, assert_ok, parameter_types, assert_err};
    use sr_primitives::{traits::{BlakeTwo256, IdentityLookup}, testing::Header};
    use sr_primitives::weights::Weight;
    use sr_primitives::Perbill;

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
    type Fees = Module<Test>;

    // This function basically just builds a genesis storage key/value store according to
    // our desired mockup.
    fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {

        let mut t = system::GenesisConfig::default().build_storage::<Test>().unwrap();

        // pre-fill balances
        balances::GenesisConfig::<Test> {
            balances: vec![(1, 100000), (2, 100000)],
            vesting: vec![],
        }.assimilate_storage(&mut t).unwrap();
        t.into()
    }

    #[test]
    fn can_change_fee_allows_all() {
        with_externalities(&mut new_test_ext(), || {
            assert_ok!(Fees::can_change_fee(123));
        });
    }

    #[test]
    fn multiple_new_fees_are_setable() {
        with_externalities(&mut new_test_ext(), || {
            let fee_key1 = <Test as system::Trait>::Hashing::hash_of(&11111);
            let fee_key2 = <Test as system::Trait>::Hashing::hash_of(&22222);

            let price1: <Test as balances::Trait>::Balance = 666;
            let price2: <Test as balances::Trait>::Balance = 777;

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
        with_externalities(&mut new_test_ext(), || {
            let fee_key = <Test as system::Trait>::Hashing::hash_of(&11111);

            let initial_price: <Test as balances::Trait>::Balance = 666;
            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, initial_price));

            let loaded_fee = Fees::fee(fee_key);
            assert_eq!(loaded_fee.price, initial_price);

            // set fee to different price, set by different account
            let new_price: <Test as balances::Trait>::Balance = 777;
            assert_ok!(Fees::set_fee(Origin::signed(2), fee_key, new_price));
            let again_loaded_fee = Fees::fee(fee_key);
            assert_eq!(again_loaded_fee.price, new_price);
        });
    }

    #[test]
    fn fee_payment_errors_if_not_set() {
        with_externalities(&mut new_test_ext(), || {
            let fee_key = <Test as system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as balances::Trait>::Balance = 90000;

            assert_err!(Fees::pay_fee(1, fee_key), "fee not found for key");

            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));

            // initial time paying will succeed as sufficient balance + fee is set
            assert_ok!(Fees::pay_fee(1, fee_key));

            //second time paying will lead to account having insufficient balance
            assert_err!(Fees::pay_fee(1, fee_key), "too few free funds in account");
        });
    }

    #[test]
    fn fee_payment_errors_if_insufficient_balance() {
        with_externalities(&mut new_test_ext(), || {
            let fee_key = <Test as system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as balances::Trait>::Balance = 90000;

            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));

            // account 3 is not endowed in the test setup
            assert_err!(Fees::pay_fee(3, fee_key), "too few free funds in account");
        });
    }

    #[test]
    fn fee_payment_subtracts_fees_from_account() {
        with_externalities(&mut new_test_ext(), || {
            let fee_key = <Test as system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as balances::Trait>::Balance = 90000;
            assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));

            // account 1 is endowed in test setup
            // initial time paying will succeed as sufficient balance + fee is set
            assert_ok!(Fees::pay_fee(1, fee_key));

            //second time paying will lead to account having insufficient balance
            assert_err!(Fees::pay_fee(1, fee_key), "too few free funds in account");
        });
    }

    #[test]
    fn fee_is_gettable() {
        with_externalities(&mut new_test_ext(), || {
            let fee_key = <Test as system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as balances::Trait>::Balance = 90000;

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
