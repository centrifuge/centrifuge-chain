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

use cfg_primitives::TRANSACTION_RECOVERY_ID;
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
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_ethereum::Config {
		/// The event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	/// Storage for nonce.
	#[pallet::storage]
	pub(crate) type Nonce<T: Config> = StorageValue<_, U256, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A call was executed.
		Executed {
			from: H160,
			to: H160,
			exit_reason: ExitReason,
			value: Vec<u8>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Trying to pop from an empty stack.
		StackUnderflow,

		/// Trying to push into a stack over stack limit.
		StackOverflow,

		/// Jump destination is invalid.
		InvalidJump,

		/// An opcode accesses memory region, but the region is invalid.
		InvalidRange,

		/// Encountered the designated invalid opcode.
		DesignatedInvalid,

		/// Call stack is too deep (runtime).
		CallTooDeep,

		/// Create opcode encountered collision (runtime).
		CreateCollision,

		/// Create init code exceeds limit (runtime).
		CreateContractLimit,

		/// Invalid opcode during execution or starting byte is 0xef. See [EIP-3541](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-3541.md).
		InvalidCode(u8),

		/// An opcode accesses external information, but the request is off
		/// offset limit (runtime).
		OutOfOffset,

		/// Execution runs out of gas (runtime).
		OutOfGas,

		/// Not enough fund to start the execution (runtime).
		OutOfFund,

		/// PC underflowed (unused).
		PCUnderflow,

		/// Attempt to create an empty account (runtime, unused).
		CreateEmpty,

		/// The operation is not supported.
		NotSupported,
		/// The trap (interrupt) is unhandled.
		UnhandledInterrupt,

		/// Machine encountered an explicit revert.
		Reverted,

		/// Unexpected result when executing a transaction.
		UnexpectedExecuteResult,

		/// Other normal errors.
		Other,
	}

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
		/// This implementation serves as a wrapper around the Ethereum pallet
		/// execute functionality. It keeps track of the nonce used for each
		/// call and builds a fake signature for executing the provided call.
		///
		/// NOTE - The execution fees are charged by the Ethereum pallet,
		/// we only have to charge for the nonce read operation.
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

			let dispatch_info = PostDispatchInfo {
				actual_weight: Some(read_weight),
				pays_fee: Pays::Yes,
			};

			match info {
				CallOrCreateInfo::Call(call_info) => {
					Self::deposit_event(Event::Executed {
						from,
						to,
						exit_reason: call_info.exit_reason.clone(),
						value: call_info.value.clone(),
					});

					match call_info.exit_reason {
						ExitReason::Succeed(_) => Ok(dispatch_info),
						ExitReason::Error(e) => Err(DispatchErrorWithPostInfo {
							post_info: dispatch_info,
							error: map_evm_error::<T>(e).into(),
						}),
						ExitReason::Revert(_) => Err(DispatchErrorWithPostInfo {
							post_info: dispatch_info,
							error: Error::<T>::Reverted.into(),
						}),
						ExitReason::Fatal(e) => Err(DispatchErrorWithPostInfo {
							post_info: dispatch_info,
							error: map_evm_fatal_error::<T>(e).into(),
						}),
					}
				}
				CallOrCreateInfo::Create(_) => Err(DispatchErrorWithPostInfo {
					post_info: dispatch_info,
					error: Error::<T>::UnexpectedExecuteResult.into(),
				}),
			}
		}
	}

	fn map_evm_error<T: Config>(e: ExitError) -> Error<T> {
		match e {
			ExitError::StackUnderflow => Error::StackUnderflow,
			ExitError::StackOverflow => Error::StackOverflow,
			ExitError::InvalidJump => Error::InvalidJump,
			ExitError::InvalidRange => Error::InvalidRange,
			ExitError::DesignatedInvalid => Error::DesignatedInvalid,
			ExitError::CallTooDeep => Error::CallTooDeep,
			ExitError::CreateCollision => Error::CreateCollision,
			ExitError::CreateContractLimit => Error::CreateContractLimit,
			ExitError::InvalidCode(opcode) => Error::InvalidCode(opcode.0),
			ExitError::OutOfOffset => Error::OutOfOffset,
			ExitError::OutOfGas => Error::OutOfGas,
			ExitError::OutOfFund => Error::OutOfFund,
			ExitError::PCUnderflow => Error::PCUnderflow,
			ExitError::CreateEmpty => Error::CreateEmpty,
			ExitError::Other(_) => Error::Other,
		}
	}

	fn map_evm_fatal_error<T: Config>(e: ExitFatal) -> Error<T> {
		match e {
			ExitFatal::NotSupported => Error::NotSupported,
			ExitFatal::UnhandledInterrupt => Error::UnhandledInterrupt,
			ExitFatal::CallErrorAsFatal(e) => map_evm_error(e),
			ExitFatal::Other(_) => Error::Other,
		}
	}
}
