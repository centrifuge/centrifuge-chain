// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
#![cfg_attr(not(feature = "std"), no_std)]

use cfg_traits::ethereum::EthereumTransactor;
use ethereum::{LegacyTransaction, TransactionAction, TransactionSignature, TransactionV2};
use fp_evm::CallOrCreateInfo;
use frame_support::{
	dispatch::{DispatchErrorWithPostInfo, PostDispatchInfo},
	pallet_prelude::*,
};
pub use pallet::*;
use pallet_evm::{ExitError, ExitFatal, ExitReason};
use sp_core::{H160, H256, U256};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

const TRANSACTION_RECOVERY_ID: u64 = 42;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_ethereum::Config {}

	/// Storage for nonce.
	#[pallet::storage]
	pub(crate) type Nonce<T: Config> = StorageValue<_, U256, ValueQuery>;

	impl<T: Config> Pallet<T> {
		fn get_transaction_signature() -> Option<TransactionSignature> {
			//TODO(cdamian): Same signature as the one in ethereum-xcm.
			TransactionSignature::new(
				TRANSACTION_RECOVERY_ID,
				H256::from_low_u64_be(1u64),
				H256::from_low_u64_be(1u64),
			)
		}
	}

	impl<T: Config> EthereumTransactor for Pallet<T> {
		fn call(
			from: H160,
			to: H160,
			data: &[u8],
			value: U256,
			gas_price: U256,
			gas_limit: U256,
		) -> DispatchResultWithPostInfo {
			let nonce = Nonce::<T>::get();
			let read_weight = T::DbWeight::get().reads(1);

			let signature =
				Pallet::<T>::get_transaction_signature().ok_or(DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo {
						actual_weight: Some(read_weight),
						pays_fee: Pays::Yes,
					},
					error: DispatchError::Other("Failed to create transaction signature"),
				})?;

			let transaction = TransactionV2::Legacy(LegacyTransaction {
				nonce,
				gas_price,
				gas_limit,
				action: TransactionAction::Call(to),
				value,
				input: data.into(),
				signature,
			});

			Nonce::<T>::put(nonce.saturating_add(U256::one()));

			// NOTE - the underlying EVM runner will charge the account derived from `from`.
			let (_target, _value, info) = pallet_ethereum::Pallet::<T>::execute(
				from,
				&transaction,
				Some(T::config().clone()),
			)
			.map_err(|e| {
				let weight = e.post_info.actual_weight.map_or(Weight::zero(), |w| w);

				DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo {
						actual_weight: Some(weight.saturating_add(read_weight)),
						pays_fee: Pays::Yes,
					},
					error: e.error,
				}
			})?;

			// The other fees related to this transaction were charged by the EVM
			// runner, we only have to charge for the nonce read operation.
			match info {
				CallOrCreateInfo::Call(call_info) => match call_info.exit_reason {
					ExitReason::Succeed(_) => Ok(PostDispatchInfo {
						actual_weight: Some(read_weight),
						pays_fee: Pays::Yes,
					}),
					ExitReason::Error(e) => Err(DispatchErrorWithPostInfo {
						post_info: PostDispatchInfo {
							actual_weight: Some(read_weight),
							pays_fee: Pays::Yes,
						},
						error: map_evm_error(e),
					}),
					ExitReason::Revert(_) => Err(DispatchErrorWithPostInfo {
						post_info: PostDispatchInfo {
							actual_weight: Some(read_weight),
							pays_fee: Pays::Yes,
						},
						error: DispatchError::Other("EVM encountered an explicit revert"),
					}),
					ExitReason::Fatal(e) => Err(DispatchErrorWithPostInfo {
						post_info: PostDispatchInfo {
							actual_weight: Some(read_weight),
							pays_fee: Pays::Yes,
						},
						error: map_evm_fatal_error(e),
					}),
				},
				CallOrCreateInfo::Create(_) => Err(DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo {
						actual_weight: Some(read_weight),
						pays_fee: Pays::Yes,
					},
					error: DispatchError::Other("unexpected execute result"),
				}),
			}
		}
	}

	fn map_evm_error(e: ExitError) -> DispatchError {
		match e {
			ExitError::StackUnderflow => DispatchError::Other("stack underflow"),
			ExitError::StackOverflow => DispatchError::Other("stack overflow"),
			ExitError::InvalidJump => DispatchError::Other("invalid jump"),
			ExitError::InvalidRange => DispatchError::Other("invalid range"),
			ExitError::DesignatedInvalid => DispatchError::Other("designated invalid"),
			ExitError::CallTooDeep => DispatchError::Other("call too deep"),
			ExitError::CreateCollision => DispatchError::Other("create collision"),
			ExitError::CreateContractLimit => DispatchError::Other("create contract limit"),
			ExitError::InvalidCode(_) => DispatchError::Other("invalid op code"),
			ExitError::OutOfOffset => DispatchError::Other("out of offset"),
			ExitError::OutOfGas => DispatchError::Other("out of gas"),
			ExitError::OutOfFund => DispatchError::Other("out of fund"),
			ExitError::PCUnderflow => DispatchError::Other("PC underflow"),
			ExitError::CreateEmpty => DispatchError::Other("create empty"),
			ExitError::Other(_) => DispatchError::Other("evm error"),
		}
	}

	fn map_evm_fatal_error(e: ExitFatal) -> DispatchError {
		match e {
			ExitFatal::NotSupported => DispatchError::Other("not supported"),
			ExitFatal::UnhandledInterrupt => DispatchError::Other("unhandled interrupt"),
			ExitFatal::CallErrorAsFatal(e) => map_evm_error(e),
			ExitFatal::Other(_) => DispatchError::Other("evm fatal error"),
		}
	}
}
