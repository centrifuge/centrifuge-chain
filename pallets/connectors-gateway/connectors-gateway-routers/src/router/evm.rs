// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

use cfg_traits::ethereum::EthereumTransactor;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure,
};
use scale_info::TypeInfo;
use sp_core::{H160, H256, U256};
use sp_runtime::traits::{BlakeTwo256, Hash};
use sp_std::{marker::PhantomData, vec::Vec};

/// A generic router used for executing EVM calls.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EVMRouter<T>
where
	T: frame_system::Config + pallet_ethereum_transaction::Config + pallet_evm::Config,
{
	pub evm_domain: EVMDomain,
	pub _marker: PhantomData<T>,
}

impl<T> EVMRouter<T>
where
	T: frame_system::Config + pallet_ethereum_transaction::Config + pallet_evm::Config,
	T::AccountId: AsRef<[u8; 32]>,
{
	/// Performs an extra check to ensure that the actual contract is deployed
	/// at the provided address and that the contract code hash matches.
	pub fn do_init(&self) -> DispatchResult {
		let code = pallet_evm::AccountCodes::<T>::get(self.evm_domain.target_contract_address);

		ensure!(
			BlakeTwo256::hash_of(&code) == self.evm_domain.target_contract_hash,
			DispatchError::Other("Target contract code does not match"),
		);

		Ok(())
	}

	/// NOTE - the sender account ID provided here will be converted to an EVM
	/// address via truncating. When the call is processed by the underlying EVM
	/// pallet, this EVM address will be converted back into a substrate account
	/// which will be charged for the transaction. This converted substrate
	/// account is not the same as the original account.
	pub fn do_send(&self, sender: T::AccountId, msg: Vec<u8>) -> DispatchResult {
		let sender_evm_address = H160::from_slice(&sender.as_ref()[0..20]);

		// TODO(cdamian): This returns a `DispatchResultWithPostInfo`. Should we
		// propagate that to another layer that will eventually charge for the
		// weight in the PostDispatchInfo?
		<pallet_ethereum_transaction::Pallet<T> as EthereumTransactor>::call(
			sender_evm_address,
			self.evm_domain.target_contract_address,
			msg.as_slice(),
			self.evm_domain.fee_values.value,
			self.evm_domain.fee_values.gas_price,
			self.evm_domain.fee_values.gas_limit,
		)
		.map_err(|e| e.error)?;

		Ok(())
	}
}

/// The EVMDomain holds all relevant information for validating and executing
/// the EVM call.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EVMDomain {
	/// The address of the contract deployed in our EVM.
	pub target_contract_address: H160,

	/// The `BlakeTwo256` hash of the target contract code.
	///
	/// This is used during router initialization to ensure that the correct
	/// contract code is used.
	pub target_contract_hash: H256,

	/// The values used when executing the EVM call.
	pub fee_values: FeeValues,
}

/// The FeeValues holds all information related to the transaction costs.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct FeeValues {
	/// The value used when executing the EVM call.
	pub value: U256,

	/// The gas price used when executing the EVM call.
	pub gas_price: U256,

	/// The gas limit used when executing the EVM call.
	pub gas_limit: U256,
}
