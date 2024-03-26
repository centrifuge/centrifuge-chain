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
use frame_support::{
	dispatch::{DispatchErrorWithPostInfo, PostDispatchInfo},
	pallet_prelude::*,
};
pub use pallet::*;
use sp_core::{H160, H256, U256};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use ethereum::ReceiptV3;
	use frame_system::pallet_prelude::OriginFor;

	use super::*;

	#[pallet::pallet]

	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_ethereum::Config
	where
		OriginFor<Self>:
			From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<Self>>>,
	{
	}

	/// Storage for nonce.
	#[pallet::storage]
	#[pallet::getter(fn nonce)]
	pub(crate) type Nonce<T: Config> = StorageValue<_, U256, ValueQuery>;

	impl<T: Config> Pallet<T>
	where
		OriginFor<T>:
			From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
	{
		pub fn get_transaction_signature() -> Option<TransactionSignature> {
			TransactionSignature::new(
				TRANSACTION_RECOVERY_ID,
				H256::from_low_u64_be(2u64),
				H256::from_low_u64_be(2u64),
			)
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		EvmExecutionFailed,
	}

	impl<T: Config> Pallet<T>
	where
		OriginFor<T>:
			From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
	{
		fn valid_code(receipt: &ReceiptV3) -> bool {
			let code = match receipt {
				ReceiptV3::Legacy(inner)
				| ReceiptV3::EIP2930(inner)
				| ReceiptV3::EIP1559(inner) => inner.status_code,
			};

			code == 1
		}
	}

	impl<T: Config> EthereumTransactor for Pallet<T>
	where
		OriginFor<T>:
			From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
	{
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

			let info = pallet_ethereum::Pallet::<T>::transact(
				pallet_ethereum::Origin::EthereumTransaction(from).into(),
				transaction,
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
			})
			.map(|dispatch_info| PostDispatchInfo {
				pays_fee: Pays::Yes,
				actual_weight: dispatch_info
					.actual_weight
					.map_or(Some(read_weight), |weight| {
						Some(weight.saturating_add(read_weight))
					}),
			})?;

			// NOTE: The Ethereum side of things never returns a DispatchError
			//       if the execution failed. But we can check that manually by
			//       querying the `Pending` storage of the pallet-ethereum.
			let pending = pallet_ethereum::Pending::<T>::get();
			let (_, _, receipt) = pending.last().ok_or(DispatchError::Other(
				"Ethereuem not adding pending storage. Unexpected.",
			))?;

			if Pallet::<T>::valid_code(receipt) {
				Ok(info)
			} else {
				Err(Error::<T>::EvmExecutionFailed.into())
			}
		}
	}
}
