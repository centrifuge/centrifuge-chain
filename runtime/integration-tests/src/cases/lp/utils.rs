// Copyright 2024 Centrifuge Foundation (centrifuge.io).
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

use std::{cmp::min, fmt::Debug};

use cfg_primitives::{AccountId, Balance, TrancheId};
use cfg_types::domain_address::DomainAddress;
use ethabi::ethereum_types::{H160, H256, U256};
use fp_evm::CallInfo;
use frame_support::traits::{OriginTrait, PalletInfo};
use frame_system::pallet_prelude::OriginFor;
use pallet_evm::ExecutionInfo;
use pallet_liquidity_pools_gateway::message::GatewayMessage;
use sp_core::Get;
use sp_runtime::{traits::EnsureAdd, DispatchError};
use staging_xcm::{
	v4::{
		Junction::{AccountKey20, GlobalConsensus, PalletInstance},
		NetworkId,
	},
	VersionedLocation,
};

use crate::{
	cases::lp::{EVM_DOMAIN_CHAIN_ID, EVM_ROUTER_ID, POOL_A, POOL_B, POOL_C},
	config::Runtime,
	utils::{accounts::Keyring, last_event, pool::get_tranche_ids},
};

/// Returns the local representation of a remote ethereum account
pub fn remote_account_of<T: Runtime>(keyring: Keyring) -> AccountId {
	DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, keyring.in_eth()).account()
}

pub const REVERT_ERR: Result<CallInfo, DispatchError> =
	Err(DispatchError::Other("EVM call failed: Revert"));

pub fn lp_asset_location<T: Runtime>(address: H160) -> VersionedLocation {
	[
		PalletInstance(
			<T as frame_system::Config>::PalletInfo::index::<pallet_liquidity_pools::Pallet<T>>()
				.unwrap()
				.try_into()
				.unwrap(),
		),
		GlobalConsensus(NetworkId::Ethereum {
			chain_id: EVM_DOMAIN_CHAIN_ID,
		}),
		AccountKey20 {
			key: address.into(),
			network: None,
		},
	]
	.into()
}

pub fn pool_a_tranche_1_id<T: Runtime>() -> TrancheId {
	*get_tranche_ids::<T>(POOL_A)
		.get(0)
		.expect("Pool A has one non-residuary tranche")
}
pub fn pool_b_tranche_1_id<T: Runtime>() -> TrancheId {
	*get_tranche_ids::<T>(POOL_B)
		.get(0)
		.expect("Pool B has two non-residuary tranches")
}
pub fn pool_b_tranche_2_id<T: Runtime>() -> TrancheId {
	*get_tranche_ids::<T>(POOL_B)
		.get(1)
		.expect("Pool B has two non-residuary tranches")
}

pub fn pool_c_tranche_1_id<T: Runtime>() -> TrancheId {
	*get_tranche_ids::<T>(POOL_C)
		.get(0)
		.expect("Pool C has one non-residuary tranche")
}

pub fn verify_outbound_failure_on_lp<T: Runtime>(to: H160) {
	let (_tx, status, _receipt) = pallet_ethereum::Pending::<T>::get()
		.last()
		.expect("Queue triggered evm tx.")
		.clone();

	// The sender is the sender account on the gateway
	assert_eq!(T::Sender::get().h160(), status.from);
	assert_eq!(status.to.unwrap().0, to.0);
	assert!(matches!(
		last_event::<T, pallet_liquidity_pools_gateway_queue::Event::<T>>(),
		pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionFailure { .. }
	));
}

pub fn verify_gateway_message_success<T: Runtime>(
	lp_message: <T as pallet_liquidity_pools_gateway::Config>::Message,
) {
	assert!(matches!(
	   last_event::<T, pallet_liquidity_pools_gateway_queue::Event::<T>>(),
	   pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionSuccess {
		  message: processed_message,
		  ..
	   } if {
		  match &processed_message {
				 GatewayMessage::Inbound{ message, .. }
				| GatewayMessage::Outbound{ message, .. } => *message == lp_message,
			  }
	   }
	));
}

pub fn process_gateway_message<T: Runtime>(
	mut verifier: impl FnMut(<T as pallet_liquidity_pools_gateway::Config>::Message),
) {
	let msgs = pallet_liquidity_pools_gateway_queue::MessageQueue::<T>::iter()
		.map(|(nonce, msg)| (nonce, msg))
		.collect::<Vec<_>>();

	// The function should panic if there is nothing to be processed.
	assert!(msgs.len() > 0, "No messages in the queue");

	msgs.into_iter().for_each(|(nonce, msg)| {
		pallet_liquidity_pools_gateway_queue::Pallet::<T>::process_message(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			nonce,
		)
		.unwrap();

		let _events = last_event::<T, pallet_liquidity_pools_gateway_queue::Event<T>>();

		match msg {
			GatewayMessage::Inbound { message, .. } => verifier(message),
			GatewayMessage::Outbound { router_id, message } => {
				assert_eq!(router_id, EVM_ROUTER_ID);
				verifier(message)
			}
		}
	});
}

pub fn to_fixed_array<const S: usize>(src: &[u8]) -> [u8; S] {
	let mut dest = [0; S];
	let len = min(src.len(), S);
	dest[..len].copy_from_slice(&src[..len]);

	dest
}

pub fn as_h160_32bytes(who: Keyring) -> [u8; 32] {
	let mut address = [0u8; 32];
	address[..20].copy_from_slice(who.in_eth().as_bytes());
	address
}

trait Input {
	fn input(&self) -> &[u8];
}

impl Input for Vec<u8> {
	fn input(&self) -> &[u8] {
		self.as_slice()
	}
}

impl<E: Debug> Input for Result<Vec<u8>, E> {
	fn input(&self) -> &[u8] {
		match self {
			Ok(arr) => arr.as_slice(),
			Err(e) => panic!("Input received error: {:?}", e),
		}
	}
}

impl<E: Debug> Input for Result<ExecutionInfo<Vec<u8>>, E> {
	fn input(&self) -> &[u8] {
		match self {
			Ok(arr) => arr.value.as_slice(),
			Err(e) => panic!("Input received error: {:?}", e),
		}
	}
}

pub trait Decoder<T> {
	fn decode(&self) -> T;
}

impl<T: Input> Decoder<(bool, u64)> for T {
	fn decode(&self) -> (bool, u64) {
		assert!(self.input().len() > 32);

		let left = &self.input()[..32];
		let right = &self.input()[32..];

		let unsigned64 = match right.len() {
			1 => u64::from(u8::from_be_bytes(to_fixed_array(&right))),
			2 => u64::from(u16::from_be_bytes(to_fixed_array(&right))),
			4 => u64::from(u32::from_be_bytes(to_fixed_array(&right))),
			8 => u64::from_be_bytes(to_fixed_array(&right)),
			// EVM stores in 32 byte slots with left-padding
			16 => u64::from_be_bytes(to_fixed_array::<8>(&right[28..])),
			32 => u64::from_be_bytes(to_fixed_array::<8>(&right[24..])),
			_ => {
				panic!("Invalid slice length for u64 derivation");
			}
		};

		(left[31] == 1u8, unsigned64)
	}
}

impl<T: Input> Decoder<H160> for T {
	fn decode(&self) -> H160 {
		assert_eq!(self.input().len(), 32usize);

		H160::from(to_fixed_array(&self.input()[12..]))
	}
}

impl<T: Input> Decoder<H256> for T {
	fn decode(&self) -> H256 {
		assert_eq!(self.input().len(), 32usize);

		H256::from(to_fixed_array(self.input()))
	}
}

impl<T: Input> Decoder<bool> for T {
	fn decode(&self) -> bool {
		assert_eq!(self.input().len(), 32usize);

		// In EVM the last byte of the U256 is set to 1 if true else to false
		self.input()[31] == 1u8
	}
}

impl<T: Input> Decoder<Balance> for T {
	fn decode(&self) -> Balance {
		assert_eq!(self.input().len(), 32usize);

		Balance::from_be_bytes(to_fixed_array(&self.input()[16..]))
	}
}

impl<T: Input> Decoder<U256> for T {
	fn decode(&self) -> U256 {
		match self.input().len() {
			1 => U256::from(u8::from_be_bytes(to_fixed_array(&self.input()))),
			2 => U256::from(u16::from_be_bytes(to_fixed_array(&self.input()))),
			4 => U256::from(u32::from_be_bytes(to_fixed_array(&self.input()))),
			8 => U256::from(u64::from_be_bytes(to_fixed_array(&self.input()))),
			16 => U256::from(u128::from_be_bytes(to_fixed_array(&self.input()))),
			32 => U256::from_big_endian(to_fixed_array::<32>(&self.input()).as_slice()),
			_ => {
				panic!("Invalid slice length for u256 derivation")
			}
		}
	}
}

impl<T: Input> Decoder<(u128, u64)> for T {
	fn decode(&self) -> (u128, u64) {
		assert!(self.input().len() >= 32);

		let left = &self.input()[..32];
		let right = &self.input()[32..];

		let unsigned128 = match left.len() {
			1 => u128::from(u8::from_be_bytes(to_fixed_array(&left))),
			2 => u128::from(u16::from_be_bytes(to_fixed_array(&left))),
			4 => u128::from(u32::from_be_bytes(to_fixed_array(&left))),
			8 => u128::from(u64::from_be_bytes(to_fixed_array(&left))),
			16 => u128::from(u128::from_be_bytes(to_fixed_array(&left))),
			32 => {
				let x = u128::from_be_bytes(to_fixed_array::<16>(&left[..16]));
				let y = u128::from_be_bytes(to_fixed_array::<16>(&left[16..]));
				x.ensure_add(y)
					.expect("Price is initialized as u128 on EVM side")
			}
			_ => {
				panic!("Invalid slice length for u128 derivation");
			}
		};

		let unsigned64 = match right.len() {
			1 => u64::from(u8::from_be_bytes(to_fixed_array(&right))),
			2 => u64::from(u16::from_be_bytes(to_fixed_array(&right))),
			4 => u64::from(u32::from_be_bytes(to_fixed_array(&right))),
			8 => u64::from_be_bytes(to_fixed_array(&right)),
			// EVM stores in 32 byte slots with left-padding
			16 => u64::from_be_bytes(to_fixed_array::<8>(&right[28..])),
			32 => u64::from_be_bytes(to_fixed_array::<8>(&right[24..])),
			_ => {
				panic!("Invalid slice length for u64 derivation");
			}
		};

		(unsigned128, unsigned64)
	}
}

impl<T: Input> Decoder<u8> for T {
	fn decode(&self) -> u8 {
		assert_eq!(self.input().len(), 32usize);

		self.input()[31]
	}
}
