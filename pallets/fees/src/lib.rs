//! # Fees pallet for runtime
//!
//! This pallet provides functionality for setting and getting fees associated with an Hash key..
//! Fees are set by FeeOrigin or RootOrigin
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_system::ensure_root;
use frame_support::{
    traits::{EnsureOrigin, Currency, ExistenceRequirement, WithdrawReasons},
    dispatch::DispatchResult,
    ensure
};

pub use pallet::*;
pub mod weights;
pub use weights::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Fee<Hash, Balance> {
    key: Hash,
    price: Balance,
}

#[frame_support::pallet]
pub mod pallet {
    // Import various types used to declare pallet in scope.
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use super::*;

    // Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
    // method.
    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config + pallet_authorship::Config {
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Required origin for changing fees
        type FeeChangeOrigin: EnsureOrigin<Self::Origin>;
        /// Type representing the weight of this pallet
        type WeightInfo: WeightInfo;
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // The genesis config type.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config>{
        pub initial_fees: Vec<(T::Hash, T::Balance)>
    }

    // The default value for the genesis config type.
    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                initial_fees: Default::default()
            }
        }
    }

    // The build of genesis for the pallet.
    #[pallet::genesis_build]
    impl <T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for fee in self.initial_fees.iter(){
                <Pallet<T>>::change_fee(fee.0, fee.1);
            }
        }
    }

    /// Stores the Fees associated with a Hash identifier
    #[pallet::storage]
    #[pallet::getter(fn fee)]
    pub(super) type Fees<T: Config> = StorageMap<_, Blake2_256, T::Hash, Fee<T::Hash, T::Balance>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        FeeChanged(T::Hash, T::Balance),
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set the given fee for the key
        #[pallet::weight(<T as pallet::Config>::WeightInfo::set_fee())]
        pub fn set_fee(origin: OriginFor<T>, key: T::Hash, new_price: T::Balance) -> DispatchResult {
            Self::can_change_fee(origin)?;
            Self::change_fee(key, new_price);
            Self::deposit_event(Event::FeeChanged(key, new_price));
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Called by any other module who wants to trigger a fee payment for a given account.
    /// The current fee price can be retrieved via Fees::price_of()
    pub fn pay_fee(from: T::AccountId, key: T::Hash) -> DispatchResult {
        ensure!(<Fees<T>>::contains_key(key), "fee not found for key");

        let single_fee = <Fees<T>>::get(key).unwrap();
        Self::pay_fee_to_author(from, single_fee.price)?;

        Ok(())
    }

    /// Burns Fee from account
    pub fn burn_fee(from: &T::AccountId, fee: T::Balance) -> DispatchResult {
        let _ = <pallet_balances::Pallet<T> as Currency<_>>::withdraw(
            from,
            fee,
            WithdrawReasons::FEE.into(),
            ExistenceRequirement::KeepAlive,
        )?;

        Ok(())
    }

    /// Pay the given fee
    pub fn pay_fee_to_author(from: T::AccountId, fee: T::Balance) -> DispatchResult {
        let value = <pallet_balances::Pallet<T> as Currency<_>>::withdraw(
            &from,
            fee,
            WithdrawReasons::FEE.into(),
            ExistenceRequirement::KeepAlive,
        )?;

        let author = <pallet_authorship::Pallet<T>>::author();
        <pallet_balances::Pallet<T> as Currency<_>>::resolve_creating(&author, value);
        Ok(())
    }

    /// Returns the current fee for the key
    pub fn price_of(key: T::Hash) -> Option<T::Balance> {
        //why this has been hashed again after passing to the function? sp_io::print(key.as_ref());
        if <Fees<T>>::contains_key(&key) {
            let single_fee = <Fees<T>>::get(&key).unwrap();
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

    /// Change the fee for the given key
    fn change_fee(key: T::Hash, fee: T::Balance) {
        let new_fee = Fee {
            key: key.clone(),
            price: fee,
        };
        <Fees<T>>::insert(key, new_fee);
    }
}
