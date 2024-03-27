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

//! Testing essential account derivations that are done in the runtime

use cfg_primitives::AccountId;
use runtime_common::apis::runtime_decl_for_account_conversion_api::AccountConversionApi;
use sp_api::{BlockT, HeaderT};
use sp_runtime::traits::{Get, Zero};
use staging_xcm::v3::{
	Junction::{AccountId32, AccountKey20, Parachain},
	Junctions::{X1, X2},
	MultiLocation, NetworkId,
};

use crate::generic::{config::Runtime, env::Env, envs::runtime_env::RuntimeEnv};

const KEY_20: [u8; 20] = [
	0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
];

const KEY_32: [u8; 32] = [
	0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
	26, 27, 28, 29, 30, 31,
];

const RANDOM_EVM_ID: u64 = 7868687u64;

const RANDOM_PARA_ID: u32 = 1230412u32;

fn network_id(chain_id: u64) -> Option<NetworkId> {
	Some(NetworkId::Ethereum { chain_id })
}

fn evm_derivation_copy(chain_id: u64) -> AccountId {
	let tag = b"EVM";
	let mut bytes = [0; 32];
	bytes[0..20].copy_from_slice(&KEY_20);
	bytes[20..28].copy_from_slice(&chain_id.to_be_bytes());
	bytes[28..31].copy_from_slice(tag);
	AccountId::new(bytes)
}

fn local_evm_account<T: Runtime>() {
	let env = RuntimeEnv::<T>::default();

	let derived = env.parachain_state(|| {
		T::Api::conversion_of(MultiLocation::new(
			0,
			X1(AccountKey20 {
				key: KEY_20,
				network: network_id(pallet_evm_chain_id::Pallet::<T>::get()),
			}),
		))
		.unwrap()
	});

	assert_eq!(
		evm_derivation_copy(env.parachain_state(pallet_evm_chain_id::Pallet::<T>::get)),
		derived
	);
}

fn lp_evm_account<T: Runtime>() {
	let env = RuntimeEnv::<T>::default();

	let derived = env.parachain_state(|| {
		T::Api::conversion_of(MultiLocation::new(
			0,
			X1(AccountKey20 {
				key: KEY_20,
				network: network_id(RANDOM_EVM_ID),
			}),
		))
		.unwrap()
	});

	assert_eq!(evm_derivation_copy(RANDOM_EVM_ID), derived);
}

fn relay_chain_account<T: Runtime>() {
	let env = RuntimeEnv::<T>::default();

	let derived = env.parachain_state(|| {
		T::Api::conversion_of(MultiLocation::new(
			1,
			X1(AccountKey20 {
				key: KEY_20,
				network: None,
			}),
		))
		.unwrap()
	});

	assert_eq!(
		AccountId::new([
			4, 59, 209, 254, 99, 95, 243, 33, 66, 81, 61, 173, 89, 50, 167, 168, 127, 205, 21, 181,
			140, 236, 38, 204, 219, 245, 163, 125, 94, 12, 60, 229
		]),
		derived
	);

	let derived = env.parachain_state(|| {
		T::Api::conversion_of(MultiLocation::new(
			1,
			X1(AccountId32 {
				id: KEY_32,
				network: None,
			}),
		))
		.unwrap()
	});

	assert_eq!(
		AccountId::new([
			254, 215, 56, 52, 116, 98, 213, 210, 66, 203, 84, 103, 189, 233, 54, 117, 31, 174, 247,
			234, 64, 173, 211, 235, 181, 10, 68, 230, 98, 50, 132, 44
		]),
		derived
	);
}

fn sibling_chain_account<T: Runtime>() {
	let env = RuntimeEnv::<T>::default();

	let derived = env.parachain_state(|| {
		T::Api::conversion_of(MultiLocation::new(
			1,
			X2(
				Parachain(RANDOM_PARA_ID),
				AccountKey20 {
					key: KEY_20,
					network: None,
				},
			),
		))
		.unwrap()
	});

	assert_eq!(
		AccountId::new([
			201, 87, 44, 251, 77, 114, 74, 143, 48, 100, 31, 110, 2, 1, 181, 223, 57, 225, 98, 105,
			223, 208, 198, 185, 81, 33, 105, 208, 64, 93, 239, 106
		]),
		derived
	);

	let derived = env.parachain_state(|| {
		T::Api::conversion_of(MultiLocation::new(
			1,
			X2(
				Parachain(RANDOM_PARA_ID),
				AccountId32 {
					id: KEY_32,
					network: None,
				},
			),
		))
		.unwrap()
	});

	assert_eq!(
		AccountId::new([
			232, 18, 32, 29, 7, 230, 102, 47, 36, 250, 204, 9, 156, 40, 170, 26, 102, 176, 9, 149,
			41, 14, 24, 80, 7, 167, 190, 125, 109, 218, 84, 152
		]),
		derived
	);
}

fn remote_account_on_relay<T: Runtime>() {
	let env = RuntimeEnv::<T>::default();

	let derived = env.parachain_state(|| {
		T::Api::conversion_of(MultiLocation::new(
			0,
			X2(
				Parachain(parachain_info::Pallet::<T>::get().into()),
				AccountId32 {
					id: KEY_32,
					network: Some(NetworkId::ByGenesis(
						frame_system::BlockHash::<T>::get(
							<<T::BlockExt as BlockT>::Header as HeaderT>::Number::zero(),
						)
						.0,
					)),
				},
			),
		))
		.unwrap()
	});

	assert_eq!(
		AccountId::new([
			158, 225, 206, 240, 254, 14, 246, 36, 122, 8, 206, 79, 16, 214, 249, 210, 196, 152,
			224, 228, 248, 52, 181, 154, 219, 40, 14, 225, 32, 91, 187, 233
		]),
		derived
	);
}

fn remote_account_on_sibling<T: Runtime>() {
	let env = RuntimeEnv::<T>::default();

	let derived = env.parachain_state(|| {
		T::Api::conversion_of(MultiLocation::new(
			1,
			X2(
				Parachain(parachain_info::Pallet::<T>::get().into()),
				AccountId32 {
					id: KEY_32,
					network: Some(NetworkId::ByGenesis(
						frame_system::BlockHash::<T>::get(
							<<T::BlockExt as BlockT>::Header as HeaderT>::Number::zero(),
						)
						.0,
					)),
				},
			),
		))
		.unwrap()
	});

	assert_eq!(
		AccountId::new([
			126, 34, 185, 2, 219, 222, 98, 177, 214, 201, 96, 61, 209, 76, 224, 101, 48, 109, 75,
			24, 52, 172, 163, 5, 23, 233, 74, 249, 105, 114, 211, 143
		]),
		derived
	);
}

crate::test_for_runtimes!(all, local_evm_account);
crate::test_for_runtimes!(all, lp_evm_account);
crate::test_for_runtimes!(all, relay_chain_account);
crate::test_for_runtimes!(all, sibling_chain_account);
crate::test_for_runtimes!(all, remote_account_on_relay);
crate::test_for_runtimes!(all, remote_account_on_sibling);
