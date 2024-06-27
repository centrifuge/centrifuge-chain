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

use frame_support::dispatch::{DispatchResult, DispatchResultWithPostInfo};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{bounded::BoundedVec, ConstU32, H160};
use sp_runtime::DispatchError;

use crate::{routers::axelar_evm::get_axelar_encoded_msg, XCMRouter, MAX_AXELAR_EVM_CHAIN_SIZE};

/// The router used for submitting a message using Axelar via
/// Moonbeam XCM.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AxelarXCMRouter<T: pallet_xcm_transactor::Config> {
	pub router: XCMRouter<T>,
	pub axelar_target_chain: BoundedVec<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>,
	pub axelar_target_contract: H160,
}

impl<T: pallet_xcm_transactor::Config> AxelarXCMRouter<T> {
	/// Calls the init function on the EVM router.
	pub fn do_init(&self) -> DispatchResult {
		self.router.do_init()
	}

	/// Encodes the message to the required format,
	/// then executes the EVM call using the generic XCM router.
	pub fn do_send(&self, sender: T::AccountId, msg: Vec<u8>) -> DispatchResultWithPostInfo {
		let contract_call = get_axelar_encoded_msg(
			msg,
			self.axelar_target_chain.clone().into_inner(),
			self.axelar_target_contract,
		)
		.map_err(|_| DispatchError::Other("encoded contract call retrieval"))?;

		self.router.do_send(sender, contract_call)
	}
}
