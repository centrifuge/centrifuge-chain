
/// Handling fees payments for specific transactions
/// Initially being hard-coded, later coming from the governance module

use support::{decl_module, decl_storage, ensure, StorageValue, StorageMap, decl_event, dispatch::Result, traits::{WithdrawReason, Currency, ExistenceRequirement}};
use runtime_primitives::traits::{Hash, As};
use parity_codec::{Encode, Decode};
use system::{ensure_signed};

// TODO tie in governance
//use super::validatorset;

/// The module's configuration trait.
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
	trait Store for Module<T: Trait> as FeesStorage {
		Fees get(fee) : map T::Hash => Fee<T::Hash, T::Balance>;
	}
}

// TODO REMOVE
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
		// TODO REMOVE
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
    pub fn pay_fee(who: T::AccountId, key: T::Hash) -> Result {
        ensure!(!<Fees<T>>::exists(key), "Fee not found for name");

        let single_fee = <Fees<T>>::get(key);

//        let bal_amount = <T::Balance as As<u64>>::sa(amount);
        let _ = <balances::Module<T> as Currency<_>>::withdraw(
            &who,
            single_fee.price,
            WithdrawReason::Fee,
            ExistenceRequirement::KeepAlive
        )?;

        Ok(())
    }

    fn can_change_fee(_who: T::AccountId) -> Result {
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
    use support::{impl_outer_origin, assert_ok};
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
        system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
    }

    #[test]
    fn can_change_fee_allows_all() {
        with_externalities(&mut new_test_ext(), || {
            assert_ok!(Fees::can_change_fee(123));
        });
    }
}
