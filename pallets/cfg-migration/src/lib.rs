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
	use cfg_types::domain_address::DomainAddress;
	use frame_support::{
		sp_runtime::traits::AccountIdConversion,
		traits::{fungibles::Mutate, tokens::Preservation, OriginTrait},
		PalletId,
	};
	use frame_system::pallet_prelude::*;
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
		type EVMChainId: Get<u64>;
		type AxelarGateway: Get<H160>;
		type AddressConverter: Convert<T::AccountId, H160>;
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
			bridge_fee: T::Balance,
			receiver: H160,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let cfg_lock_account = T::CfgLockAccount::get().into_account_truncating();
			let iou = T::IouCfg::get();
			let native_cfg = T::NativeCfg::get();

			// Get user's full CFG balance
			let total_balance = T::Tokens::balance(native_cfg, &who);
			ensure!(total_balance >= bridge_fee, Error::<T>::InsufficientBalance);

			let transfer_amount = total_balance.ensure_sub(&bridge_fee)?;

			// Transfer full balance to lock account first
			T::Tokens::transfer(
				native_cfg,
				&who,
				&cfg_lock_account,
				total_balance,
				Preservation::Expendable,
			)?;

			// Mint IOU for actual transfer amount
			T::Tokens::mint_into(iou, &cfg_lock_account, transfer_amount)?;

			// Pay Axelar fees via EVM call
			Self::pay_axelar_fee(fee, &cfg_lock_account)?;

			// Initiate cross-chain transfer
			pallet_liquidity_pools::Pallet::<T>::transfer(
				OriginFor::<T>::signed(cfg_lock_account.clone()),
				T::IouCfg::get(),
				DomainAddress::Evm(T::EVMChainId::get(), receiver),
				transfer_amount,
			)?;

			Self::deposit_event(Event::CfgMigrationInitiated {
				sender: who,
				receiver,
				amount: transfer_amount,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn pay_axelar_fee(fee: T::Balance, from_account: &T::AccountId) -> DispatchResult {
			let axelar_gateway = T::AxelarGateway::get();
			let evm_account = T::AddressConverter::convert(from_account.clone());

			// Convert fee to EVM compatible format
			let fee_u256: U256 = fee.into();

			// Construct call to Axelar's CallContract
			let call_data = ethabi::encode(&[
				Token::String("ethereum".into()),
				Token::String("0x".into()), // Placeholder for destination address
				Token::Bytes(vec![]),       // Empty payload for example
			]);

			// Dispatch EVM call
			pallet_evm::Pallet::<T>::call(
				evm_account,
				axelar_gateway,
				call_data,
				fee_u256,
				1000000u64.into(),         // Example gas limit
				U256::from(1_000_000_000), // Example gas price
				None,
				None,
				vec![],
			)
			.map_err(|_| Error::<T>::AxelarFeePaymentFailed)?;

			Ok(())
		}
	}
}
