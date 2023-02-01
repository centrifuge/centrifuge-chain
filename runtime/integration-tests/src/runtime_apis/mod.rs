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
mod pools;
mod rewards;

use std::sync::Arc;

use cfg_primitives::{AuraId, CFG};
use frame_support::traits::GenesisBuild;
use fudge::{
	digest::{DigestCreator, DigestProvider, FudgeAuraDigest},
	inherent::{FudgeInherentParaParachain, FudgeInherentTimestamp},
	primitives::ParaId,
	state::StateProvider,
	StandaloneBuilder, TWasmExecutor,
};
use polkadot_primitives::v2::PersistedValidationData;
use sc_client_api::{HeaderBackend, StorageProof};
use sc_executor::WasmExecutor;
use sc_service::TFullClient;
use sp_api::ProvideRuntimeApi as _;
use sp_consensus_slots::SlotDuration;
use sp_core::{
	sr25519,
	sr25519::{Pair, Public},
	Pair as TraitPair, H256,
};
use sp_inherents::CreateInherentDataProviders;
use sp_runtime::{generic::BlockId, traits::IdentifyAccount, BuildStorage, Storage};
use tokio::runtime::Handle;
use xcm_emulator::ParachainInherentData;

use crate::{
	chain::{
		centrifuge,
		centrifuge::{Runtime, PARA_ID},
	},
	pools::utils::{
		genesis::{default_balances, register_default_asset},
		time::START_DATE,
	},
};

/// The type that CreatesInherentDataProviders for the para-chain.
/// As a new-type here as otherwise the TestEnv is badly
/// readable.
#[allow(unused)]
type Cidp = Box<
	dyn CreateInherentDataProviders<
		centrifuge::Block,
		(),
		InherentDataProviders = (
			FudgeInherentTimestamp,
			sp_consensus_aura::inherents::InherentDataProvider,
			FudgeInherentParaParachain,
		),
	>,
>;

/// The type creates digests for the chains.
#[allow(unused)]
type Dp = Box<dyn DigestCreator<centrifuge::Block> + Send + Sync>;

type ApiRef<'a> = sp_api::ApiRef<'a, <TFullClient<centrifuge::Block, centrifuge::RuntimeApi, TWasmExecutor> as sp_api::ProvideRuntimeApi<centrifuge::Block>>::Api>;

fn create_builder(
	handle: Handle,
	genesis: Option<impl BuildStorage>,
) -> StandaloneBuilder<centrifuge::Block, centrifuge::RuntimeApi, Cidp, Dp> {
	let mut state = StateProvider::new(centrifuge::WASM_BINARY.expect("Wasm is build. Qed."));
	state.insert_storage(
		pallet_aura::GenesisConfig::<centrifuge::Runtime> {
			authorities: vec![AuraId::from(sr25519::Public([0u8; 32]))],
		}
		.build_storage()
		.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
	);

	let mut storage = Storage::default();
	// Add default balances
	default_balances::<Runtime>(&mut storage);
	// Register default assets
	register_default_asset::<Runtime>(&mut storage);

	state.insert_storage(storage);

	if let Some(storage) = genesis {
		state.insert_storage(storage);
	}

	let mut init = fudge::initiator::default(handle);
	init.with_genesis(Box::new(state));

	let para_id = ParaId::from(centrifuge::PARA_ID);
	let instance_id = FudgeInherentTimestamp::create_instance(
		std::time::Duration::from_secs(12),
		Some(std::time::Duration::from_millis(START_DATE)),
	);

	let cidp = Box::new(move |_parent: H256, ()| {
		async move {
			let timestamp = FudgeInherentTimestamp::get_instance(instance_id)
				.expect("Instances is initialized");

			let slot =
					sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
						timestamp.current_time(),
						SlotDuration::from_millis(std::time::Duration::from_secs(12).as_millis() as u64),
					);
			// Dummy data for relay-inherent
			let inherent = ParachainInherentData {
				validation_data: PersistedValidationData {
					parent_head: Default::default(),
					relay_parent_number: 1,
					relay_parent_storage_root: H256::zero(),
					max_pov_size: 0,
				},
				relay_chain_state: StorageProof::empty(),
				downward_messages: vec![],
				horizontal_messages: Default::default(),
			};
			let relay_para_inherent = FudgeInherentParaParachain::new(inherent);
			Ok((timestamp, slot, relay_para_inherent))
		}
	});
	let dp = |clone_client: Arc<
		sc_service::TFullClient<centrifuge::Block, centrifuge::RuntimeApi, TWasmExecutor>,
	>| {
		Box::new(move |parent, inherents| {
			let client = clone_client.clone();

			async move {
				let aura = FudgeAuraDigest::<
					centrifuge::Block,
					sc_service::TFullClient<
						centrifuge::Block,
						centrifuge::RuntimeApi,
						TWasmExecutor,
					>,
				>::new(&*client);

				let digest = aura.build_digest(&parent, &inherents).await?;
				Ok(digest)
			}
		})
	};

	StandaloneBuilder::<_, _, Cidp, Dp>::new(init, |client| (cidp, dp(client)))
}

pub struct ApiEnv {
	builder: StandaloneBuilder<centrifuge::Block, centrifuge::RuntimeApi, Cidp, Dp>,
}

impl ApiEnv {
	pub fn new(handle: Handle) -> Self {
		crate::pools::utils::logs::init_logs();

		Self {
			builder: create_builder(handle, Some(Storage::default())),
		}
	}

	pub fn new_with_genesis(handle: Handle, genesis: impl BuildStorage) -> Self {
		// TODO: Actually make a lot of the utils in pools not specific to pools
		//       testing. Like init logs, creating builder and so on.
		crate::pools::utils::logs::init_logs();
		Self {
			builder: create_builder(handle, Some(genesis)),
		}
	}

	pub fn with_api<F>(&self, exec: F) -> &Self
	where
		F: FnOnce(ApiRef, BlockId<centrifuge::Block>),
	{
		let client = self.builder.client();
		let api = client.runtime_api();
		let best_hash = BlockId::hash(self.builder.client().info().best_hash);

		exec(api, best_hash);

		self
	}

	pub fn startup<F>(&mut self, start_up: F) -> &mut Self
	where
		F: FnOnce(),
	{
		self.builder.with_mut_state(start_up).unwrap();

		self
	}
}
