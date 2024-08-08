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
//
//! # Liquidity Pools Gateway Routers
//!
//! This crate contains the `DomainRouters` used by the Liquidity Pools Gateway
//! pallet.
//!
//! The routers can be used to communicate with:
//!
//! Axelar - via EVM and XCM (through Moonbeam).
//!
//! Moonbeam - via XCM.
#![cfg_attr(not(feature = "std"), no_std)]

use cfg_traits::{ethereum::EthereumTransactor, liquidity_pools::Router};
use frame_support::{
	ensure,
	pallet_prelude::{DispatchError, DispatchResult, DispatchResultWithPostInfo},
};
use frame_system::pallet_prelude::OriginFor;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{H160, H256, U256};
use sp_runtime::traits::{BlakeTwo256, Hash};
use sp_std::{marker::PhantomData, vec::Vec};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod routers {
	pub mod axelar_evm;
}

pub use routers::axelar_evm::AxelarEVMRouter;

/// Maximum size allowed for a byte representation of an Axelar EVM chain
/// string, as found below:
/// <https://docs.axelar.dev/dev/reference/mainnet-chain-names>
/// <https://docs.axelar.dev/dev/reference/testnet-chain-names>
pub const MAX_AXELAR_EVM_CHAIN_SIZE: u32 = 16;

const AXELAR_FUNCTION_NAME: &str = "callContract";
const AXELAR_DESTINATION_CHAIN_PARAM: &str = "destinationChain";
const AXELAR_DESTINATION_CONTRACT_ADDRESS_PARAM: &str = "destinationContractAddress";
const AXELAR_PAYLOAD_PARAM: &str = "payload";

/// The routers used for outgoing messages.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum DomainRouter<T>
where
	T: pallet_ethereum_transaction::Config + pallet_evm::Config,
	T::AccountId: AsRef<[u8; 32]>,
	OriginFor<T>:
		From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
{
	AxelarEVM(AxelarEVMRouter<T>),
}

impl<T> Router for DomainRouter<T>
where
	T: pallet_ethereum_transaction::Config + pallet_evm::Config,
	T::AccountId: AsRef<[u8; 32]>,
	OriginFor<T>:
		From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
{
	type Hash = T::Hash;
	type Sender = T::AccountId;

	fn init(&self) -> DispatchResult {
		match self {
			DomainRouter::AxelarEVM(r) => r.do_init(),
		}
	}

	fn send(&self, sender: Self::Sender, message: Vec<u8>) -> DispatchResultWithPostInfo {
		match self {
			DomainRouter::AxelarEVM(r) => r.do_send(sender, message),
		}
	}

	fn hash(&self) -> Self::Hash {
		match self {
			DomainRouter::EthereumXCM(r) => r.hash(),
			DomainRouter::AxelarEVM(r) => r.hash(),
			DomainRouter::AxelarXCM(r) => r.hash(),
		}
	}
}

/// A generic router used for executing EVM calls.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EVMRouter<T>
where
	T: pallet_ethereum_transaction::Config + pallet_evm::Config,
	OriginFor<T>:
		From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
{
	pub evm_domain: EVMDomain,
	pub _marker: PhantomData<T>,
}

impl<T> EVMRouter<T>
where
	T: pallet_ethereum_transaction::Config + pallet_evm::Config,
	OriginFor<T>:
		From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
{
	pub fn new(evm_domain: EVMDomain) -> Self {
		Self {
			evm_domain,
			_marker: Default::default(),
		}
	}
}

impl<T> EVMRouter<T>
where
	T: pallet_ethereum_transaction::Config + pallet_evm::Config,
	T::AccountId: AsRef<[u8; 32]>,
	OriginFor<T>:
		From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
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
	pub fn do_send(&self, sender: T::AccountId, msg: Vec<u8>) -> DispatchResultWithPostInfo {
		let sender_evm_address = H160::from_slice(&sender.as_ref()[0..20]);

		<pallet_ethereum_transaction::Pallet<T> as EthereumTransactor>::call(
			sender_evm_address,
			self.evm_domain.target_contract_address,
			msg.as_slice(),
			self.evm_domain.fee_values.value,
			self.evm_domain.fee_values.gas_price,
			self.evm_domain.fee_values.gas_limit,
		)
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
