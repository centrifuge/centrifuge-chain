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

use cfg_primitives::{
	AccountId as CentrifugeAccountId, Address as CentrifugeAddress, Nonce as CentrifugeNonce,
};
use parity_scale_codec::Encode;
use polkadot_core_primitives::{
	AccountId as RelayAccountId, BlockId as RelayBlockId, Nonce as RelayNonce,
};
use sc_client_api::client::BlockBackend;
use sp_core::H256;
use sp_runtime::{
	generic::{Era, SignedPayload},
	MultiSignature,
};

use crate::{
	chain::{
		centrifuge,
		centrifuge::{
			Runtime as CentrifugeRuntime, RuntimeCall as CentrifugeCall,
			SignedExtra as CentrifugeSignedExtra, UncheckedExtrinsic as CentrifugeUnchecked,
		},
		relay,
		relay::{
			Address as RelayAddress, Runtime as RelayRuntime, RuntimeCall as RelayCall,
			SignedExtra as RelaySignedExtra, UncheckedExtrinsic as RelayUnchecked,
		},
	},
	utils::{accounts::Keyring, env::TestEnv},
};

/// Generates an signed-extrinisc for centrifuge-chain.
///
/// **NOTE: Should not be used if the TesteEnv::sign_and_submit() interface is
/// also used with         the same `who` as the sender**
pub fn xt_centrifuge(
	env: &TestEnv,
	who: Keyring,
	nonce: cfg_primitives::Nonce,
	call: centrifuge::RuntimeCall,
) -> Result<centrifuge::UncheckedExtrinsic, Box<dyn std::error::Error>> {
	let client = env.centrifuge.client();

	let genesis_hash = client
		.block_hash(0)
		.expect("ESSENTIAL: Genesis MUST be avilable.")
		.unwrap();
	let (spec_version, tx_version) = {
		let version = client
			.runtime_version_at(client.chain_info().best_hash)
			.unwrap();
		(version.spec_version, version.transaction_version)
	};

	env.centrifuge
		.with_state(|| sign_centrifuge(who, nonce, call, spec_version, tx_version, genesis_hash))
		.map_err(|e| e.into())
}

/// Generates an signed-extrinisc for relay-chain.
///
/// **NOTE: Should not be used if the TesteEnv::sign_and_submit() interface is
/// also used with         the same `who` as the sender**
pub fn xt_relay(
	env: &TestEnv,
	who: Keyring,
	nonce: RelayNonce,
	call: relay::RuntimeCall,
) -> Result<relay::UncheckedExtrinsic, Box<dyn std::error::Error>> {
	let client = env.relay.client();

	let genesis_hash = client
		.block_hash(0)
		.expect("ESSENTIAL: Genesis MUST be avilable.")
		.expect("ESSENTIAL: Genesis MUST be avilable.");
	let (spec_version, tx_version) = {
		let version = client
			.runtime_version_at(client.chain_info().best_hash)
			.unwrap();
		(version.spec_version, version.transaction_version)
	};

	env.relay
		.with_state(|| sign_relay(who, nonce, call, spec_version, tx_version, genesis_hash))
		.map_err(|e| e.into())
}

fn signed_extra_centrifuge(nonce: cfg_primitives::Nonce) -> CentrifugeSignedExtra {
	(
		frame_system::CheckNonZeroSender::<CentrifugeRuntime>::new(),
		frame_system::CheckSpecVersion::<CentrifugeRuntime>::new(),
		frame_system::CheckTxVersion::<CentrifugeRuntime>::new(),
		frame_system::CheckGenesis::<CentrifugeRuntime>::new(),
		frame_system::CheckEra::<CentrifugeRuntime>::from(Era::mortal(256, 0)),
		frame_system::CheckNonce::<CentrifugeRuntime>::from(nonce),
		frame_system::CheckWeight::<CentrifugeRuntime>::new(),
		pallet_transaction_payment::ChargeTransactionPayment::<CentrifugeRuntime>::from(0),
		runtime_common::transfer_filter::PreBalanceTransferExtension::<CentrifugeRuntime>::new(),
	)
}

fn sign_centrifuge(
	who: Keyring,
	nonce: cfg_primitives::Nonce,
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

fn signed_extra_relay(nonce: RelayNonce) -> RelaySignedExtra {
	(
		frame_system::CheckNonZeroSender::<RelayRuntime>::new(),
		frame_system::CheckSpecVersion::<RelayRuntime>::new(),
		frame_system::CheckTxVersion::<RelayRuntime>::new(),
		frame_system::CheckGenesis::<RelayRuntime>::new(),
		frame_system::CheckMortality::<RelayRuntime>::from(Era::mortal(256, 0)),
		frame_system::CheckNonce::<RelayRuntime>::from(nonce),
		frame_system::CheckWeight::<RelayRuntime>::new(),
		pallet_transaction_payment::ChargeTransactionPayment::<RelayRuntime>::from(0),
	)
}

fn sign_relay(
	who: Keyring,
	nonce: RelayNonce,
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

/// Retrieves the latest centrifuge nonce for a given account.
///
/// **NOTE: Should not be used if the TesteEnv::sign_and_submit() interface is
/// also used with         the same `who` as the sender**
pub fn nonce_centrifuge(env: &TestEnv, who: Keyring) -> cfg_primitives::Nonce {
	env.centrifuge
		.with_state(|| {
			nonce::<CentrifugeRuntime, CentrifugeAccountId, CentrifugeNonce>(
				who.clone().to_account_id().into(),
			)
		})
		.expect("ESSENTIAL: Nonce must be retrievable.")
}

/// Retrieves the latest relay nonce for a given account.
///
/// **NOTE: Should not be used if the TesteEnv::sign_and_submit() interface is
/// also used with         the same `who` as the sender**
pub fn nonce_relay(env: &TestEnv, who: Keyring) -> RelayNonce {
	env.relay
		.with_state(|| {
			nonce::<RelayRuntime, RelayAccountId, RelayNonce>(who.clone().to_account_id().into())
		})
		.expect("ESSENTIAL: Nonce must be retrievable.")
}

fn nonce<Runtime, AccountId, Nonce>(who: AccountId) -> Nonce
where
	Runtime: frame_system::Config,
	AccountId: Into<<Runtime as frame_system::Config>::AccountId>,
	Nonce: From<<Runtime as frame_system::Config>::Nonce>,
{
	frame_system::Pallet::<Runtime>::account_nonce(who.into()).into()
}

mod tests {
	use fudge::primitives::Chain;
	use pallet_balances::Call as BalancesCall;
	use parity_scale_codec::Encode;
	use sp_runtime::Storage;
	use tokio::runtime::Handle;

	use super::{nonce_centrifuge, xt_centrifuge};
	use crate::{
		chain::{
			centrifuge,
			centrifuge::{Runtime, PARA_ID},
		},
		utils::{accounts::Keyring, env, genesis},
	};

	#[tokio::test]
	async fn extrinsics_works() {
		let mut genesis = Storage::default();
		genesis::default_balances::<Runtime>(&mut genesis);
		let mut env = env::test_env_with_centrifuge_storage(Handle::current(), genesis);

		let to: cfg_primitives::Address = Keyring::Bob.into();
		let xt = xt_centrifuge(
			&env,
			Keyring::Alice,
			nonce_centrifuge(&env, Keyring::Alice),
			centrifuge::RuntimeCall::Balances(BalancesCall::transfer {
				dest: to,
				value: 100 * cfg_primitives::constants::CFG,
			}),
		)
		.unwrap();
		env.append_extrinsic(Chain::Para(PARA_ID), xt.encode())
			.unwrap();

		let (alice_before, bob_before) = env
			.with_state(Chain::Para(PARA_ID), || {
				(
					frame_system::Pallet::<Runtime>::account(Keyring::Alice.to_account_id()),
					frame_system::Pallet::<Runtime>::account(Keyring::Bob.to_account_id()),
				)
			})
			.unwrap();

		env.evolve().unwrap();

		let (alice_after, bob_after) = env
			.with_state(Chain::Para(PARA_ID), || {
				(
					frame_system::Pallet::<Runtime>::account(Keyring::Alice.to_account_id()),
					frame_system::Pallet::<Runtime>::account(Keyring::Bob.to_account_id()),
				)
			})
			.unwrap();

		// Need to account for fees here
		assert!(
			alice_after.data.free <= alice_before.data.free - 100 * cfg_primitives::constants::CFG
		);
		assert_eq!(
			bob_after.data.free,
			bob_before.data.free + 100 * cfg_primitives::constants::CFG
		);

		env.evolve().unwrap();

		let (alice_after, bob_after) = env
			.with_state(Chain::Para(PARA_ID), || {
				(
					frame_system::Pallet::<Runtime>::account(Keyring::Alice.to_account_id()),
					frame_system::Pallet::<Runtime>::account(Keyring::Bob.to_account_id()),
				)
			})
			.unwrap();

		// Need to account for fees here
		assert!(
			alice_after.data.free <= alice_before.data.free - 100 * cfg_primitives::constants::CFG
		);
		assert_eq!(
			bob_after.data.free,
			bob_before.data.free + 100 * cfg_primitives::constants::CFG
		);
	}
}
