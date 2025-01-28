#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;

mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use pallet::*;
pub use weights::WeightInfo;

const MAX_AXELAR_EVM_CHAIN_SIZE: u32 = 16;
pub type ChainName = BoundedVec<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::liquidity_pools::{AxelarGasService, LpMessageSerializer};
	use cfg_types::domain_address::DomainAddress;
	use frame_support::{
		sp_runtime::traits::{AccountIdConversion, EnsureSub, Zero},
		traits::{
			fungibles::{Inspect, Mutate},
			tokens::Preservation,
			OriginTrait,
		},
		PalletId,
	};
	use frame_system::pallet_prelude::*;
	use pallet_liquidity_pools::Message;
	use sp_core::H160;

	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_liquidity_pools::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type CfgLockAccount: Get<PalletId>;

		type IouCfg: Get<Self::CurrencyId>;

		type NativeCfg: Get<Self::CurrencyId>;

		type ReceiverEVMChainId: Get<u64>;

		type DestinationAxelarChainName: Get<ChainName>;

		type GasPaymentService: AxelarGasService<
			Middleware = ChainName,
			Origin = DomainAddress,
			Message = Vec<u8>,
		>;

		/// The sender account that will be used in the OutboundQueue
		/// implementation.
		#[pallet::constant]
		type Sender: Get<DomainAddress>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emit when user can not pay for the bridge fee
		InsufficientBalance,
		/// Emit when user has no balance to transfer, after fee
		ZeroTransfer,
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
		#[pallet::weight(<T as Config>::WeightInfo::migrate())]
		#[pallet::call_index(0)]
		pub fn migrate(
			origin: OriginFor<T>,
			bridge_fee: T::Balance,
			receiver: H160,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let mut weight = Weight::zero();

			let cfg_lock_account = T::CfgLockAccount::get().into_account_truncating();
			let iou_cfg = T::IouCfg::get();
			let native_cfg = T::NativeCfg::get();
			let receiver = DomainAddress::Evm(T::ReceiverEVMChainId::get(), receiver);

			// Get user's full CFG balance
			let total_balance = T::Tokens::balance(native_cfg, &who);
			weight.saturating_accrue(T::DbWeight::get().reads(1));
			ensure!(total_balance >= bridge_fee, Error::<T>::InsufficientBalance);

			let transfer_amount = total_balance.ensure_sub(bridge_fee)?;

			ensure!(!transfer_amount.is_zero(), Error::<T>::ZeroTransfer);

			// Transfer sending balance to lock account first
			T::Tokens::transfer(
				native_cfg,
				&who,
				&cfg_lock_account,
				transfer_amount,
				Preservation::Expendable,
			)?;
			weight.saturating_accrue(T::DbWeight::get().reads_writes(3, 2));

			// Transfer bridge fee to sending account
			T::Tokens::transfer(
				native_cfg,
				&who,
				&T::Sender::get().account(),
				bridge_fee,
				Preservation::Expendable,
			)?;
			weight.saturating_accrue(T::DbWeight::get().reads_writes(3, 2));

			// Mint IOU for actual transfer amount
			T::Tokens::mint_into(iou_cfg, &cfg_lock_account, transfer_amount)?;
			weight.saturating_accrue(T::DbWeight::get().reads_writes(2, 2));

			if !bridge_fee.is_zero() {
				// Pay bridge fee
				weight.saturating_accrue(
					T::GasPaymentService::pay_fees(
						T::DestinationAxelarChainName::get(),
						T::Sender::get(),
						Message::TransferAssets {
							currency: pallet_liquidity_pools::Pallet::<T>::try_get_general_index(
								iou_cfg,
							)?,
							receiver: receiver.bytes(),
							amount: transfer_amount.into(),
						}
						.serialize(),
						bridge_fee.into(),
					)?
					.actual_weight
					.unwrap_or_default(),
				);
			}

			// Initiate cross-chain transfer
			pallet_liquidity_pools::Pallet::<T>::transfer(
				OriginFor::<T>::signed(cfg_lock_account.clone()),
				iou_cfg,
				receiver.clone(),
				transfer_amount,
			)?;
			weight.saturating_accrue(weights::default_defensive_weight());

			Self::deposit_event(Event::CfgMigrationInitiated {
				sender: who,
				receiver: receiver.h160(),
				amount: transfer_amount,
			});

			Ok(Some(weight).into())
		}
	}
}
