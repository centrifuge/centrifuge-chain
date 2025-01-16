#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    dispatch::DispatchResult, pallet_prelude::*, traits::Currency,
};

#[cfg(test)]
mod mock;

mod weights;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::traits::OriginTrait;
    use sp_core::H160;
    use cfg_types::domain_address::DomainAddress;
    use frame_support::PalletId;
    use frame_support::sp_runtime::traits::AccountIdConversion;
    use frame_system::pallet_prelude::*;

    type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    type CurrencyIdFor<T> = <T as pallet_liquidity_pools::Config>::CurrencyId;


    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_liquidity_pools::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type Currency: Currency<Self::AccountId, Balance = Self::Balance>;
        type PalletAccount: Get<PalletId>;
        type IouCfg: Get<CurrencyIdFor<Self>>;
        type EVMChainId: Get<u64>;
        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CfgMigrationInitiated {
            sender: T::AccountId,
            receiver: H160,
            amount: BalanceOf<T>,
        },
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(<T as pallet::Config>::WeightInfo::migrate())]
        #[pallet::call_index(0)]
        pub fn migrate(
            origin: OriginFor<T>,
            cfg_amount: BalanceOf<T>,
            receiver: H160,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let pallet = T::PalletAccount::get().into_account_truncating();
            T::Currency::transfer(&who, &pallet, cfg_amount, frame_support::traits::ExistenceRequirement::AllowDeath)?;

            let domain_address = DomainAddress::Evm(T::EVMChainId::get(), receiver);
            let origin = OriginFor::<T>::signed(pallet);

            pallet_liquidity_pools::Pallet::<T>::transfer(origin, T::IouCfg::get(), domain_address, cfg_amount)?;

            Self::deposit_event(Event::CfgMigrationInitiated{
                sender: who,
                receiver,
                amount: cfg_amount,
            });

            Ok(())
        }
    }
}

