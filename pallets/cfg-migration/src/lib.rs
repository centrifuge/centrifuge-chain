#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;
mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

const MAX_AXELAR_EVM_CHAIN_SIZE: u32 = 16;
pub type ChainName = BoundedVec<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>;

#[frame_support::pallet]
pub mod pallet {
	use cfg_types::domain_address::DomainAddress;
	use frame_support::{
		sp_runtime::traits::{AccountIdConversion, EnsureSub, Zero},
		traits::{
			fungibles::{Inspect, Mutate},
			tokens::Preservation,
			OriginTrait,
		},
		weights::constants::RocksDbWeight,
		PalletId,
	};
	use frame_system::pallet_prelude::*;
	use sp_core::H160;

	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_liquidity_pools::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type CfgLockAccount: Get<PalletId>;

		type IouCfg: Get<Self::CurrencyId>;

		type NativeCfg: Get<Self::CurrencyId>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	pub type FeeReceiver<T: Config> =
		StorageValue<_, T::AccountId, ResultQuery<Error<T>::FeeReceiverNotSet>>;

	#[pallet::storage]
	pub type FeeAmount<T: Config> =
		StorageValue<_, T::Balance, ResultQuery<Error<T>::FeeAmountNotSet>>;

	#[pallet::error]
	pub enum Error<T> {
		/// Emit when user can not pay for the bridge fee
		InsufficientBalance,
		/// Emit when user has no balance to transfer, after fee
		ZeroTransfer,
		/// Emit when the fee receiver is not set
		FeeReceiverNotSet,
		/// Emit when the fee amount is not set
		FeeAmountNotSet,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CfgMigrationInitiated {
			sender: T::AccountId,
			receiver: H160,
			amount: T::Balance,
		},
		CfgMigrationFeePayed {
			sender: T::AccountId,
			receiver: T::AccountId,
			amount: T::Balance,
		},
		FeeReceiverSet(T::AccountId),
		FeeAmountSet(T::Balance),
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(<T as Config>::WeightInfo::migrate())]
		#[pallet::call_index(0)]
		pub fn migrate(origin: OriginFor<T>, receiver: DomainAddress) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let cfg_lock_account = T::CfgLockAccount::get().into_account_truncating();
			let iou_cfg = T::IouCfg::get();
			let native_cfg = T::NativeCfg::get();

			let fee = FeeAmount::<T>::get()?;

			// Get user's full CFG balance
			let full_balance = T::Tokens::balance(native_cfg, &who);
			ensure!(full_balance >= fee, Error::<T>::InsufficientBalance);

			let transfer_amount = full_balance.ensure_sub(fee)?;
			ensure!(!transfer_amount.is_zero(), Error::<T>::ZeroTransfer);

			T::Tokens::transfer(
				native_cfg,
				&who,
				&FeeReceiver::<T>::get()?,
				fee,
				Preservation::Expendable,
			)?;

			Self::deposit_event(Event::CfgMigrationFeePayed {
				sender: who.clone(),
				receiver: FeeReceiver::<T>::get()?,
				amount: fee,
			});

			// Transfer sending balance to lock account first
			T::Tokens::transfer(
				native_cfg,
				&who,
				&cfg_lock_account,
				transfer_amount,
				Preservation::Expendable,
			)?;

			// Mint IOU for actual transfer amount
			T::Tokens::mint_into(iou_cfg, &cfg_lock_account, transfer_amount)?;

			// Initiate cross-chain transfer
			pallet_liquidity_pools::Pallet::<T>::transfer(
				OriginFor::<T>::signed(cfg_lock_account.clone()),
				iou_cfg,
				receiver.clone(),
				transfer_amount,
			)?;

			Self::deposit_event(Event::CfgMigrationInitiated {
				sender: who,
				receiver: receiver.h160(),
				amount: transfer_amount,
			});

			Ok(())
		}

		#[pallet::weight(Weight::from_parts(50_000_000, 512).saturating_add(RocksDbWeight::get().writes(1)))]
		#[pallet::call_index(1)]
		pub fn set_fee_receiver(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			FeeReceiver::<T>::put(who.clone());

			Self::deposit_event(Event::FeeReceiverSet(who));

			Ok(())
		}

		#[pallet::weight(Weight::from_parts(50_000_000, 512).saturating_add(RocksDbWeight::get().writes(1)))]
		#[pallet::call_index(2)]
		pub fn set_fee_amount(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			FeeAmount::<T>::put(amount);

			Self::deposit_event(Event::FeeAmountSet(amount));

			Ok(())
		}
	}
}
