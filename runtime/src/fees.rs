
/// Handling fees payments for specific transactions
/// Initially being hard-coded, later coming from the governance module

use support::{decl_module, decl_storage, ensure, StorageMap, decl_event, dispatch::Result, traits::{WithdrawReason, Currency, ExistenceRequirement}};
use runtime_primitives::traits::{Hash};
use parity_codec::{Encode, Decode};
use system::{ensure_signed};

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

		pub fn set_fee(origin, new_price: T::Balance, key: T::Hash) -> Result {
            let sender = ensure_signed(origin)?;
			Self::can_change_fee(sender.clone())?;

             let new_fee = Fee{
                 key: key,
                 price: new_price,
             };
			 <Fees<T>>::insert(key, new_fee);

			Self::deposit_event(RawEvent::FeeChanged(sender, key, new_price));
            Ok(())
        }
	}
}

impl<T: Trait> Module<T> {
    // Called by any other module who wants to trigger a fee payment
    // for a given account.
    // The fee price can be retrieved via Fees::fee()
    pub fn pay_fee(who: T::AccountId, key: T::Hash) -> Result {
        ensure!(<Fees<T>>::exists(key), "fee not found for key");

        let single_fee = <Fees<T>>::get(key);

        let _ = <balances::Module<T> as Currency<_>>::withdraw(
            &who,
            single_fee.price,
            WithdrawReason::Fee,
            ExistenceRequirement::KeepAlive
        )?;

        Ok(())
    }

    fn can_change_fee(_who: T::AccountId) -> Result {
        //TODO add auth who can change fees
//        ensure!(<validatorset::Module<T>>::is_validator(who), "Not authorized to change fees.");
        Ok(())
    }
}


/// tests for this module
#[cfg(test)]
mod tests {
    use super::*;

    use runtime_io::with_externalities;
    use primitives::{H256, Blake2Hasher};
    use support::{impl_outer_origin, assert_ok, assert_err};
    use runtime_primitives::{
        BuildStorage,
        traits::{BlakeTwo256, IdentityLookup},
        testing::{Digest, DigestItem, Header}
    };

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
    impl Trait for Test {
        type Event = ();
    }
    impl balances::Trait for Test {
        type Balance = u64;
        type OnFreeBalanceZero = ();
        type OnNewAccount = ();
        type Event = ();
        type TransactionPayment = ();
        type DustRemoval = ();
        type TransferPayment = ();
    }
    type Fees = Module<Test>;


    // This function basically just builds a genesis storage key/value store according to
    // our desired mockup.
    fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
        let mut t = system::GenesisConfig::<Test>::default().build_storage().unwrap().0;

        // pre-fill balances
        t.extend(
            balances::GenesisConfig::<Test>{
                balances: vec![(1, 100000), (2, 100000)],
                transaction_base_fee: 0,
                transaction_byte_fee: 0,
                existential_deposit: 1,
                transfer_fee: 0,
                creation_fee: 0,
                vesting: vec![]
            }.build_storage().unwrap().0
        );

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
            let fee_key1= <Test as system::Trait>::Hashing::hash_of(&11111);
            let fee_key2= <Test as system::Trait>::Hashing::hash_of(&22222);

            let price1: <Test as balances::Trait>::Balance = 666;
            let price2: <Test as balances::Trait>::Balance = 777;

            assert_ok!(Fees::set_fee(Origin::signed(1), price1, fee_key1));
            assert_ok!(Fees::set_fee(Origin::signed(1), price2, fee_key2));

            let loaded_fee1 = Fees::fee(fee_key1);
            assert_eq!(loaded_fee1.price, price1);

            let loaded_fee2 = Fees::fee(fee_key2);
            assert_eq!(loaded_fee2.price, price2);
        });
    }

    #[test]
    fn fee_is_re_setable() {
        with_externalities(&mut new_test_ext(), || {
            let fee_key= <Test as system::Trait>::Hashing::hash_of(&11111);

            let initial_price: <Test as balances::Trait>::Balance = 666;
            assert_ok!(Fees::set_fee(Origin::signed(1), initial_price, fee_key));

            let loaded_fee = Fees::fee(fee_key);
            assert_eq!(loaded_fee.price, initial_price);

            // set fee to different price, set by different account
            let new_price: <Test as balances::Trait>::Balance = 777;
            assert_ok!(Fees::set_fee(Origin::signed(2), new_price, fee_key));
            let again_loaded_fee = Fees::fee(fee_key);
            assert_eq!(again_loaded_fee.price, new_price);

        });
    }

    #[test]
    fn fee_payment_errors_if_not_set() {
        with_externalities(&mut new_test_ext(), || {
            let fee_key= <Test as system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as balances::Trait>::Balance = 90000;

            assert_err!(Fees::pay_fee(1, fee_key), "fee not found for key");

            assert_ok!(Fees::set_fee(Origin::signed(1), fee_price, fee_key));

            // initial time paying will succeed as sufficient balance + fee is set
            assert_ok!(Fees::pay_fee(1, fee_key));

            //second time paying will lead to account having insufficient balance
            assert_err!(Fees::pay_fee(1, fee_key), "too few free funds in account");
        });
    }

    #[test]
    fn fee_payment_errors_if_insufficient_balance() {
        with_externalities(&mut new_test_ext(), || {
            let fee_key= <Test as system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as balances::Trait>::Balance = 90000;

            assert_ok!(Fees::set_fee(Origin::signed(1), fee_price, fee_key));

            // account 3 is not endowed in the test setup
            assert_err!(Fees::pay_fee(3, fee_key), "too few free funds in account");
        });
    }

    #[test]
    fn fee_payment_subtracts_fees_from_account() {
        with_externalities(&mut new_test_ext(), || {
            let fee_key= <Test as system::Trait>::Hashing::hash_of(&111111);
            let fee_price: <Test as balances::Trait>::Balance = 90000;
            assert_ok!(Fees::set_fee(Origin::signed(1), fee_price, fee_key));

            // account 1 is endowed in test setup
            // initial time paying will succeed as sufficient balance + fee is set
            assert_ok!(Fees::pay_fee(1, fee_key));

            //second time paying will lead to account having insufficient balance
            assert_err!(Fees::pay_fee(1, fee_key), "too few free funds in account");
        });
    }
}


