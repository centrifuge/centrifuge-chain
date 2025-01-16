#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{dispatch::DispatchResult, pallet_prelude::*};

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
	use cfg_types::domain_address::DomainAddress;
	use frame_support::sp_runtime::traits::AccountIdConversion;
	use frame_support::traits::fungibles::Mutate;
	use frame_support::traits::tokens::Preservation;
	use frame_support::traits::OriginTrait;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use sp_core::H160;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_liquidity_pools::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type CfgLockAccount: Get<PalletId>;
		type IouCfg: Get<Self::CurrencyId>;
		type NativeCfg: Get<Self::CurrencyId>;
		type EVMChainId: Get<u64>;
		type WeightInfo: WeightInfo;
	}
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CfgMigrationInitiated {
			sender: T::AccountId,
			receiver: H160,
			amount: T::Balance,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(<T as pallet::Config>::WeightInfo::migrate())]
		#[pallet::call_index(0)]
		pub fn migrate(
			origin: OriginFor<T>,
			cfg_amount: T::Balance,
			receiver: H160,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let cfg_lock_account = T::CfgLockAccount::get().into_account_truncating();
			let iou = T::IouCfg::get();

			T::Tokens::transfer(
				T::NativeCfg::get(),
				&who,
				&cfg_lock_account,
				cfg_amount,
				Preservation::Expendable,
			)?;
			T::Tokens::mint_into(iou, &cfg_lock_account, cfg_amount)?;

			pallet_liquidity_pools::Pallet::<T>::transfer(
				OriginFor::<T>::signed(cfg_lock_account),
				T::IouCfg::get(),
				DomainAddress::Evm(T::EVMChainId::get(), receiver),
				cfg_amount,
			)?;

			Self::deposit_event(Event::CfgMigrationInitiated {
				sender: who,
				receiver,
				amount: cfg_amount,
			});

			Ok(())
		}
	}
}
