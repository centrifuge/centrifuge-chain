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

//! Utilities for creating extrinsics
#![allow(unused)]

use crate::chain::centrifuge::{
	Call as CentrifugeCall, Runtime as CentrifugeRuntime, SignedExtra as CentrifugeSignedExtra,
	UncheckedExtrinsic as CentrifugeUnchecked,
};
use crate::chain::relay::{
	Address as RelayAddress, Call as RelayCall, Runtime as RelayRuntime,
	SignedExtra as RelaySignedExtra, UncheckedExtrinsic as RelayUnchecked,
};
use crate::chain::{centrifuge, relay};
use crate::pools::utils::{accounts::Keyring, env::TestEnv};
use codec::Encode;
use node_primitives::Index as RelayIndex;
use polkadot_core_primitives::{AccountId as RelayAccountId, BlockId as RelayBlockId};
use runtime_common::{
	AccountId as CentrifugeAccountId, Address as CentrifugeAddress, Index as CentrifugeIndex,
};
use sc_client_api::client::BlockBackend;
use sp_core::H256;
use sp_runtime::{
	generic::{Era, SignedPayload},
	MultiSignature,
};

pub fn ext_centrifuge(
	env: &TestEnv,
	who: Keyring,
	nonce: centrifuge::Index,
	call: centrifuge::Call,
) -> Result<centrifuge::UncheckedExtrinsic, ()> {
	let client = env.centrifuge.client();

	let genesis_hash = client
		.block_hash(0)
		.expect("ESSENTIAL: Genesis MUST be avilable.")
		.unwrap();
	let best_block_id = centrifuge::BlockId::number(client.chain_info().best_number);
	let (spec_version, tx_version) = {
		let version = client.runtime_version_at(&best_block_id).unwrap();
		(version.spec_version, version.transaction_version)
	};

	env.centrifuge
		.with_state(|| sign_centrifuge(who, nonce, call, spec_version, tx_version, genesis_hash))
		.map_err(|_| ())
}

pub fn ext_relay(
	env: &TestEnv,
	who: Keyring,
	nonce: RelayIndex,
	call: relay::Call,
) -> Result<relay::UncheckedExtrinsic, ()> {
	let client = env.relay.client();

	let genesis_hash = client
		.block_hash(0)
		.expect("ESSENTIAL: Genesis MUST be avilable.")
		.expect("ESSENTIAL: Genesis MUST be avilable.");
	let best_block_id = RelayBlockId::number(client.chain_info().best_number);
	let (spec_version, tx_version) = {
		let version = client.runtime_version_at(&best_block_id).unwrap();
		(version.spec_version, version.transaction_version)
	};

	env.relay
		.with_state(|| sign_relay(who, nonce, call, spec_version, tx_version, genesis_hash))
		.map_err(|_| ())
}

pub fn nonce_centrifuge(env: &TestEnv, who: Keyring) -> centrifuge::Index {
	env.centrifuge
		.with_state(|| {
			nonce::<CentrifugeRuntime, CentrifugeAccountId, CentrifugeIndex>(
				who.clone().to_account_id().into(),
			)
		})
		.expect("ESSENTIAL: Nonce must be retrievable.")
}

pub fn nonce_relay(env: &TestEnv, who: Keyring) -> RelayIndex {
	env.relay
		.with_state(|| {
			nonce::<RelayRuntime, RelayAccountId, RelayIndex>(who.clone().to_account_id().into())
		})
		.expect("ESSENTIAL: Nonce must be retrievable.")
}

fn nonce<Runtime, AccountId, Index>(who: AccountId) -> Index
where
	Runtime: frame_system::Config,
	AccountId: Into<<Runtime as frame_system::Config>::AccountId>,
	Index: From<<Runtime as frame_system::Config>::Index>,
{
	frame_system::Pallet::<Runtime>::account_nonce(who.into()).into()
}

fn signed_extra_centrifuge(nonce: centrifuge::Index) -> CentrifugeSignedExtra {
	(
		frame_system::CheckNonZeroSender::<CentrifugeRuntime>::new(),
		frame_system::CheckSpecVersion::<CentrifugeRuntime>::new(),
		frame_system::CheckTxVersion::<CentrifugeRuntime>::new(),
		frame_system::CheckGenesis::<CentrifugeRuntime>::new(),
		frame_system::CheckEra::<CentrifugeRuntime>::from(Era::mortal(256, 0)),
		frame_system::CheckNonce::<CentrifugeRuntime>::from(nonce),
		frame_system::CheckWeight::<CentrifugeRuntime>::new(),
		pallet_transaction_payment::ChargeTransactionPayment::<CentrifugeRuntime>::from(0),
	)
}

fn sign_centrifuge(
	who: Keyring,
	nonce: centrifuge::Index,
	call: CentrifugeCall,
	spec_version: u32,
	tx_version: u32,
	genesis_hash: H256,
) -> CentrifugeUnchecked {
	let extra = signed_extra_centrifuge(nonce);
	let additional = (
		(),
		spec_version,
		tx_version,
		genesis_hash,
		genesis_hash.clone(),
		(),
		(),
		(),
	);
	let raw_payload = SignedPayload::from_raw(call.clone(), extra.clone(), additional);
	let signature = MultiSignature::Sr25519(raw_payload.using_encoded(|payload| who.sign(payload)));

	CentrifugeUnchecked::new_signed(
		call,
		CentrifugeAddress::Id(who.to_account_id()),
		signature,
		extra,
	)
}

fn signed_extra_relay(nonce: RelayIndex) -> RelaySignedExtra {
	(
		frame_system::CheckNonZeroSender::<RelayRuntime>::new(),
		frame_system::CheckSpecVersion::<RelayRuntime>::new(),
		frame_system::CheckTxVersion::<RelayRuntime>::new(),
		frame_system::CheckGenesis::<RelayRuntime>::new(),
		frame_system::CheckMortality::<RelayRuntime>::from(Era::mortal(256, 0)),
		frame_system::CheckNonce::<RelayRuntime>::from(nonce),
		frame_system::CheckWeight::<RelayRuntime>::new(),
		pallet_transaction_payment::ChargeTransactionPayment::<RelayRuntime>::from(0),
		#[cfg(not(feature = "runtime-development"))]
		polkadot_runtime_common::claims::PrevalidateAttests::<RelayRuntime>::new(),
	)
}

fn sign_relay(
	who: Keyring,
	nonce: RelayIndex,
	call: RelayCall,
	spec_version: u32,
	tx_version: u32,
	genesis_hash: H256,
) -> RelayUnchecked {
	let extra = signed_extra_relay(nonce);
	let additional = (
		(),
		spec_version,
		tx_version,
		genesis_hash.clone(),
		genesis_hash,
		(),
		(),
		(),
	);
	let raw_payload = SignedPayload::from_raw(call.clone(), extra.clone(), additional);
	let signature = MultiSignature::Sr25519(raw_payload.using_encoded(|payload| who.sign(payload)));

	RelayUnchecked::new_signed(
		call,
		RelayAddress::Id(who.to_account_id()),
		signature,
		extra,
	)
}
