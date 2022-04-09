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

//! Utilities to create a relay-chain-parachain setup
use crate::chain::centrifuge::{
	Block as CentrifugeBlock, BlockNumber, Event, Runtime, RuntimeApi as CentrifugeRtApi, PARA_ID,
	WASM_BINARY as CentrifugeCode,
};
use crate::chain::relay::{Runtime as RelayRt, RuntimeApi as RelayRtApi, WASM_BINARY as RelayCode};
use crate::pools::utils::accounts::{Keyring, NonceManager};
use crate::pools::utils::extrinsics::{xt_centrifuge, xt_relay};
use crate::pools::utils::{logs, time::START_DATE};
use codec::{Decode, Encode};
use frame_support::traits::GenesisBuild;
use frame_system::EventRecord;
use fudge::digest::FudgeBabeDigest;
use fudge::primitives::{Chain, PoolState};
use fudge::{
	digest::DigestCreator,
	inherent::{
		CreateInherentDataProviders, FudgeDummyInherentRelayParachain, FudgeInherentParaParachain,
		FudgeInherentTimestamp,
	},
	EnvProvider, ParachainBuilder, RelaychainBuilder,
};
pub use macros::*;
use polkadot_core_primitives::{Block as RelayBlock, Header as RelayHeader};
use polkadot_parachain::primitives::Id as ParaId;
use runtime_common::Index;

use sc_executor::{WasmExecutionMethod, WasmExecutor};
use sc_service::TaskManager;
use sp_consensus_babe::digests::CompatibleDigestItem;
use sp_core::H256;
use sp_runtime::{generic::BlockId, DigestItem, Storage};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;
/*
($expression:expr, $(|)? $( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {
		match $expression {
			$( $pattern )|+ $( if $guard )? => true,
			_ => false
		}
 */
pub mod macros {
	/* // TODO: Implement this assert
	/// A macro for checking if a specifc event was contained in the given range of blocks
	/// Panics if this was not the case
	macro_rules! assert_events {
		($env:expr, $chain:expr, $event:expr, $range:expr, $(|)? $( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {{
			use frame_system::EventRecord as __hidden_EventRecord;
			use sp_core::H256 as __hidden_H256;
			use codec::Decode as _;

			let scale_events = $env.events($chain, $range).expect("Failed fetching events");
			let event_records: Vec<__hidden_EventRecord<Event, H256>> = scale_events
				.into_iter()
				.map(|scale_record| __hidden_EventRecord::<$event, __hidden_H256>::decode(&mut scale_record.as_slice())
					.expect("Decoding from chain data does not fail. qed"))
				.collect();

			let matches = |event: &Event| {
				match *event {
					$( $pattern )|+ $( if $guard )? => true,
					_ => false
				}
			};

			let mut searched_events = Vec::new();
			for record in event_records {
				if matches(&record.event) {
					searched_events.push(record.event);
				}
			}

			searched_events
		}};
	}
	 */

	/// A macro that helps retrieving specific events with a filter
	/// This is useful as the general interface of the TestEnv return
	/// scale-encoded events.
	macro_rules! events {
		($env:expr, $chain:expr, $event:ty, $range:expr, $(|)? $( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {{
			use frame_system::EventRecord as __hidden_EventRecord;
			use sp_core::H256 as __hidden_H256;
			use codec::Decode as _;

			let scale_events = $env.events($chain, $range).expect("Failed fetching events");
			let event_records: Vec<__hidden_EventRecord<Event, __hidden_H256>> = scale_events
				.into_iter()
				.map(|scale_record| __hidden_EventRecord::<$event, __hidden_H256>::decode(&mut scale_record.as_slice())
					.expect("Decoding from chain data does not fail. qed"))
				.collect();

			let matches = |event: &Event| {
				match *event {
            		$( $pattern )|+ $( if $guard )? => true,
            		_ => false
				}
			};

			let mut searched_events = Vec::new();
			for record in event_records {
				if matches(&record.event) {
					searched_events.push(record.event);
				}
			}

			searched_events
		}};
	}

	/// A macro that helps including the given calls into a chain
	/// and to progress a chain until all of them are included
	macro_rules! run {
		($env:expr, $chain:expr, $state:expr, $who:expr, $($calls:expr),*) => {{
				use crate::chain::centrifuge::Call as __hidden_include_Call;
				use codec::Encode as _;

				trait CallAssimilator {
					fn assimilate(self, calls: &mut Vec<__hidden_include_Call>);
				}

				impl CallAssimilator for Vec<__hidden_include_Call> {
					fn assimilate(self, calls: &mut Vec<__hidden_include_Call>) {
						calls.extend(self);
					}
				}

				impl CallAssimilator for __hidden_include_Call {
					fn assimilate(self, calls: &mut Vec<__hidden_include_Call>) {
						calls.push(self)
					}
				}

				let mut calls = Vec::new();
				$(
				  $calls.assimilate(&mut calls);
				)*

				let sign_and_submit_res = $env.batch_sign_and_submit($chain, $who, calls.into_iter().map(|call| call.encode()).collect());
				assert!(sign_and_submit_res.is_ok());

				let evolve_res = $env.evolve_till($chain, $state);
				assert!(evolve_res.is_ok())
			}
		};
	}
	// Need to export after definition.
	pub(crate) use events;
	pub(crate) use run;
}

#[derive(Clone, Copy)]
pub enum ChainState {
	PoolEmpty,
	PoolMax(usize),
	EvolvedBy(u64),
}

#[cfg(not(feature = "runtime-benchmarks"))]
/// HostFunctions that do not include benchmarking specific host functions
type CentrifugeHF = sp_io::SubstrateHostFunctions;
#[cfg(feature = "runtime-benchmarks")]
/// Host functions that include benchmarking specific functionalities
type CentrifugeHF = sc_executor::sp_wasm_interface::ExtendedHostFunctions<
	sp_io::SubstrateHostFunctions,
	frame_benchmarking::benchmarking::HostFunctions,
>;

/// Basic supstrate host functions
type HF = sp_io::SubstrateHostFunctions;

/// The type that CreatesInherentDataProviders for the relay-chain.
/// As a new-type here as otherwise the TestEnv is badly
/// readable.
#[allow(unused)]
type RelayCidp = Box<
	dyn CreateInherentDataProviders<
		RelayBlock,
		(),
		InherentDataProviders = (
			FudgeInherentTimestamp,
			sp_consensus_babe::inherents::InherentDataProvider,
			sp_authorship::InherentDataProvider<RelayHeader>,
			FudgeDummyInherentRelayParachain<RelayHeader>,
		),
	>,
>;

/// The type that CreatesInherentDataProviders for the para-chain.
/// As a new-type here as otherwise the TestEnv is badly
/// readable.
#[allow(unused)]
type CentrifugeCidp = Box<
	dyn CreateInherentDataProviders<
		CentrifugeBlock,
		(),
		InherentDataProviders = (
			FudgeInherentTimestamp,
			sp_consensus_babe::inherents::InherentDataProvider,
			FudgeInherentParaParachain,
		),
	>,
>;

/// The type creates digests for the chains.
#[allow(unused)]
type Dp = Box<dyn DigestCreator + Send + Sync>;

/// A struct that stores all events that have been generated
/// since we are building blocks locally here.
pub struct EventsStorage {
	pub centrifuge: HashMap<BlockNumber, Vec<EventRecord<Event, H256>>>,
}

impl EventsStorage {
	pub fn new() -> Self {
		Self {
			centrifuge: HashMap::new(),
		}
	}
}

pub enum EventRange {
	All,
	Range(BlockNumber, BlockNumber),
	Latest,
}

#[fudge::companion]
pub struct TestEnv {
	#[fudge::relaychain]
	pub relay: RelaychainBuilder<RelayBlock, RelayRtApi, RelayRt, RelayCidp, Dp, HF>,
	#[fudge::parachain(PARA_ID)]
	pub centrifuge:
		ParachainBuilder<CentrifugeBlock, CentrifugeRtApi, CentrifugeCidp, Dp, CentrifugeHF>,
	nonce_manager: Arc<Mutex<NonceManager>>,
	pub events: Arc<Mutex<EventsStorage>>,
}

// NOTE: Nonce management is a known issue when interacting with a chain and wanting
//       to submit a lot of extrinsic. This interface eases this issues.
//
// Upon usage of this API:
// *
impl TestEnv {
	pub fn events(&self, chain: Chain, range: EventRange) -> Result<Vec<Vec<u8>>, ()> {
		match chain {
			Chain::Relay => todo!("Implement events fetching for relay"),
			Chain::Para(id) => match id {
				_ if id == PARA_ID => {
					let latest = self
						.centrifuge
						.with_state(|| frame_system::Pallet::<Runtime>::block_number())
						.map_err(|_| ())?;

					match range {
						EventRange::Latest => self.events_centrifuge(latest),
						EventRange::All => {
							let mut events = Vec::new();
							for block in 0..latest + 1 {
								events.extend(self.events_centrifuge(block)?)
							}

							Ok(events)
						}
						EventRange::Range(from, to) => {
							let mut events = Vec::new();
							for block in from..to + 1 {
								events.extend(self.events_centrifuge(block)?)
							}

							Ok(events)
						}
					}
				}
				_ => Err(()),
			},
		}
	}

	fn events_centrifuge(&self, at: BlockNumber) -> Result<Vec<Vec<u8>>, ()> {
		self.centrifuge
			.with_state_at(BlockId::Number(at), || {
				frame_system::Pallet::<Runtime>::events()
			})
			.map_err(|_| ())
			.map(|records| records.into_iter().map(|record| record.encode()).collect())
	}

	/// Returns the next nonce to be used
	/// **WARN: Increases the nonce counter on `NonceManager**
	fn fetch_add_nonce(&mut self, chain: Chain, who: Keyring) -> Index {
		let manager = self.nonce_manager.clone();
		self.with_state(chain, || {
			manager
				.lock()
				.expect("Locking nonce manager must not fail.")
				.fetch_add(chain, who)
		})
		.expect("Essential: Nonce need to be retrievable and incrementable")
	}

	/// Returns the next nonce to be used. Does NOT increase counter in
	/// `NonceManager`
	fn nonce(&mut self, chain: Chain, who: Keyring) -> Index {
		let manager = self.nonce_manager.clone();
		self.with_state(chain, || {
			manager
				.lock()
				.expect("Locking nonce manager must not fail.")
				.nonce(chain, who)
		})
		.expect("Essential: Nonce need to be retrievable")
	}

	/// Does increase counter in `NonceManager`. MUST only be called
	/// if a previously `nonce` has been called for a given `Keyring`
	/// `Chain` combination.
	fn incr_nonce(&mut self, chain: Chain, who: Keyring) {
		let manager = self.nonce_manager.clone();
		self.with_state(chain, || {
			manager
				.lock()
				.expect("Locking nonce manager must not fail.")
				.incr(chain, who)
				.expect("Essential: Nonce need to be incrementable")
		})
		.expect("Essential: Nonce need to be retrievable")
	}

	/// Resets the current nonce-manager
	///
	/// General notes on usage of this function:
	/// * should be used when an extrinsics fails -> nonces are out of sync
	/// * should be used when a signed extrinsic is dropped -> nonces are out of sync
	pub fn clear_nonces(&mut self) {
		self.nonce_manager = Arc::new(Mutex::new(NonceManager::new()));
	}

	/// Signs a given call for the given chain. Should only be used if the extrinsic really
	/// should be submitted afterwards.
	/// **NOTE: This will increase the stored nonce of an account**
	pub fn sign(&mut self, chain: Chain, who: Keyring, call: Vec<u8>) -> Result<Vec<u8>, ()> {
		let nonce = self.fetch_add_nonce(chain, who);
		match chain {
			Chain::Relay => Ok(xt_relay(
				self,
				who,
				nonce,
				Decode::decode(&mut call.as_slice()).map_err(|_| ())?,
			)?
			.encode()),
			Chain::Para(id) => match id {
				_ if id == PARA_ID => Ok(xt_centrifuge(
					self,
					who,
					nonce,
					Decode::decode(&mut call.as_slice()).map_err(|_| ())?,
				)?
				.encode()),
				_ => Err(()),
			},
		}
	}

	/// Submits a previously signed extrinsics to the pool of the respective chain.
	pub fn submit(&mut self, chain: Chain, xt: Vec<u8>) -> Result<(), ()> {
		self.append_extrinsic(chain, xt)
	}

	/// Signs and submits an extrinsic to the given chain. Will take the nonce for the account
	/// from the `NonceManager`.
	pub fn sign_and_submit(&mut self, chain: Chain, who: Keyring, call: Vec<u8>) -> Result<(), ()> {
		let nonce = self.nonce(chain, who);
		let xt = match chain {
			Chain::Relay => xt_relay(
				self,
				who,
				nonce,
				Decode::decode(&mut call.as_slice()).map_err(|_| ())?,
			)?
			.encode(),
			Chain::Para(id) => match id {
				_ if id == PARA_ID => xt_centrifuge(
					self,
					who,
					nonce,
					Decode::decode(&mut call.as_slice()).map_err(|_| ())?,
				)?
				.encode(),
				_ => return Err(()),
			},
		};

		self.append_extrinsic(chain, xt)?;
		self.incr_nonce(chain, who);
		Ok(())
	}

	/// Signs and submits a batch of extrinsic to the given chain. Will take the nonce for the account
	/// from the `NonceManager`.
	///
	/// Returns early of an extrinsic fails to be submitted.
	pub fn batch_sign_and_submit(
		&mut self,
		chain: Chain,
		who: Keyring,
		calls: Vec<Vec<u8>>,
	) -> Result<(), ()> {
		for call in calls {
			self.sign_and_submit(chain, who, call)?;
		}

		Ok(())
	}

	pub fn evolve_till(&mut self, chain: Chain, till_state: ChainState) -> Result<(), ()> {
		match chain {
			Chain::Relay => match till_state {
				ChainState::EvolvedBy(blocks) => pass_n(self, blocks / 2),
				ChainState::PoolEmpty => self.evolve_till_pool_xts_relay(0),
				ChainState::PoolMax(max) => self.evolve_till_pool_xts_relay(max),
			},
			Chain::Para(id) => match id {
				_ if id == PARA_ID => match till_state {
					ChainState::EvolvedBy(blocks) => pass_n(self, blocks),
					ChainState::PoolEmpty => self.evolve_till_pool_xts_centrifuge(0),
					ChainState::PoolMax(max) => self.evolve_till_pool_xts_centrifuge(max),
				},
				_ => unreachable!("No other parachain supported currently."),
			},
		}
	}

	fn evolve_till_pool_xts_centrifuge(&mut self, xts: usize) -> Result<(), ()> {
		let state = self.centrifuge.pool_state();
		let mut curr_xts = match state {
			PoolState::Empty => return Ok(()),
			PoolState::Busy(curr_xts) => curr_xts,
		};

		if curr_xts <= xts {
			return Ok(());
		} else {
			while curr_xts > xts {
				self.evolve()?;
				let event_storage = self.events.clone();
				self.centrifuge.with_state(|| {
					let mut storage = event_storage
						.lock()
						.expect("Must not fail getting event-storage");
					let block = frame_system::Pallet::<Runtime>::block_number();
					storage
						.centrifuge
						.insert(block, frame_system::Pallet::<Runtime>::events());
				});

				curr_xts = match self.centrifuge.pool_state() {
					PoolState::Empty => return Ok(()),
					PoolState::Busy(curr_xts) => curr_xts,
				};
			}
		}

		Ok(())
	}

	fn evolve_till_pool_xts_relay(&mut self, xts: usize) -> Result<(), ()> {
		let state = self.relay.pool_state();
		let mut curr_xts = match state {
			PoolState::Empty => return Ok(()),
			PoolState::Busy(curr_xts) => curr_xts,
		};

		if curr_xts <= xts {
			return Ok(());
		} else {
			while curr_xts > xts {
				self.evolve()?;
				curr_xts = match self.relay.pool_state() {
					PoolState::Empty => return Ok(()),
					PoolState::Busy(curr_xts) => curr_xts,
				};
			}
		}

		Ok(())
	}
}

#[allow(unused)]
pub fn test_env_default(manager: &TaskManager) -> TestEnv {
	test_env(manager, None, None)
}

#[allow(unused)]
pub fn test_env_with_relay_storage(manager: &TaskManager, storage: Storage) -> TestEnv {
	test_env(manager, Some(storage), None)
}

#[allow(unused)]
pub fn test_env_with_centrifuge_storage(manager: &TaskManager, storage: Storage) -> TestEnv {
	test_env(manager, None, Some(storage))
}

#[allow(unused)]
pub fn test_env_with_both_storage(
	manager: &TaskManager,
	relay_storage: Storage,
	centrifuge_storage: Storage,
) -> TestEnv {
	test_env(manager, Some(relay_storage), Some(centrifuge_storage))
}

fn test_env(
	manager: &TaskManager,
	relay_storage: Option<Storage>,
	centrifuge_storage: Option<Storage>,
) -> TestEnv {
	logs::init_logs();
	// Build relay-chain builder
	let relay = {
		sp_tracing::enter_span!(sp_tracing::Level::DEBUG, "Relay - StartUp");
		let mut provider = EnvProvider::<
			RelayBlock,
			RelayRtApi,
			WasmExecutor<sp_io::SubstrateHostFunctions>,
		>::empty();

		// We need to HostConfiguration and use the default here.
		provider.insert_storage(
			polkadot_runtime_parachains::configuration::GenesisConfig::<RelayRt>::default()
				.build_storage()
				.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
		);

		provider.insert_storage(
			frame_system::GenesisConfig {
				code: RelayCode.expect("ESSENTIAL: Relay WASM is some.").to_vec(),
			}
			.build_storage::<RelayRt>()
			.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
		);

		if let Some(storage) = relay_storage {
			provider.insert_storage(storage);
		}

		let (client, backend) = provider.init_default(
			WasmExecutor::new(WasmExecutionMethod::Interpreted, Some(8), 8, None, 2),
			Box::new(manager.spawn_handle()),
		);
		let client = Arc::new(client);
		let clone_client = client.clone();

		let cidp = Box::new(move |parent: H256, ()| {
			let client = clone_client.clone();
			let parent_header = client
				.header(&BlockId::Hash(parent.clone()))
				.expect("ESSENTIAL: Relay CIDP must not fail.")
				.expect("ESSENTIAL: Relay CIDP must not fail.");

			async move {
				let uncles =
					sc_consensus_uncles::create_uncles_inherent_data_provider(&*client, parent)?;

				let timestamp = FudgeInherentTimestamp::new(
					0,
					std::time::Duration::from_secs(6),
					Some(std::time::Duration::from_millis(START_DATE)),
				);

				let slot =
					sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_duration(
						timestamp.current_time(),
						std::time::Duration::from_secs(6),
					);

				let relay_para_inherent = FudgeDummyInherentRelayParachain::new(parent_header);
				Ok((timestamp, slot, uncles, relay_para_inherent))
			}
		});

		let dp = Box::new(move || async move {
			let mut digest = sp_runtime::Digest::default();

			let slot_duration = pallet_babe::Pallet::<RelayRt>::slot_duration();
			digest.push(<DigestItem as CompatibleDigestItem>::babe_pre_digest(
				FudgeBabeDigest::pre_digest(
					FudgeInherentTimestamp::get_instance(0).current_time(),
					std::time::Duration::from_millis(slot_duration),
				),
			));

			Ok(digest)
		});

		RelaychainBuilder::<_, _, RelayRt, RelayCidp, Dp, HF>::new(
			manager, backend, client, cidp, dp,
		)
	};

	// Build parachain-builder
	let centrifuge = {
		sp_tracing::enter_span!(sp_tracing::Level::DEBUG, "Centrifuge - StartUp");
		let mut provider =
			EnvProvider::<CentrifugeBlock, CentrifugeRtApi, WasmExecutor<CentrifugeHF>>::with_code(
				CentrifugeCode.unwrap(),
			);

		provider.insert_storage(
			frame_system::GenesisConfig {
				code: CentrifugeCode
					.expect("ESSENTIAL: Centrifuge WASM is some.")
					.to_vec(),
			}
			.build_storage::<RelayRt>()
			.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
		);

		if let Some(storage) = centrifuge_storage {
			provider.insert_storage(storage);
		}

		let (client, backend) = provider.init_default(
			WasmExecutor::new(WasmExecutionMethod::Interpreted, Some(8), 8, None, 2),
			Box::new(manager.spawn_handle()),
		);
		let client = Arc::new(client);
		let para_id = ParaId::from(PARA_ID);
		let inherent_builder = relay.inherent_builder(para_id.clone());

		let cidp = Box::new(move |_parent: H256, ()| {
			let inherent_builder_clone = inherent_builder.clone();
			async move {
				let timestamp = FudgeInherentTimestamp::new(
					1,
					std::time::Duration::from_secs(12),
					Some(std::time::Duration::from_millis(START_DATE)),
				);

				let slot =
					sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_duration(
						timestamp.current_time(),
						std::time::Duration::from_secs(12),
					);
				let inherent = inherent_builder_clone
					.parachain_inherent()
					.await
					.expect("ESSENTIAL: ParachainInherent from RelayBuilder must not fail.");
				let relay_para_inherent = FudgeInherentParaParachain::new(inherent);
				Ok((timestamp, slot, relay_para_inherent))
			}
		});
		let dp = Box::new(move || async move { Ok(sp_runtime::Digest::default()) });

		ParachainBuilder::<_, _, CentrifugeCidp, Dp, CentrifugeHF>::new(
			manager, backend, client, cidp, dp,
		)
	};

	TestEnv::new(
		relay,
		centrifuge,
		Arc::new(Mutex::new(NonceManager::new())),
		Arc::new(Mutex::new(EventsStorage::new())),
	)
	.expect("ESSENTIAL: Creating new TestEnv instance must not fail.")
}

pub fn task_manager(tokio_handle: Handle) -> TaskManager {
	TaskManager::new(tokio_handle, None).expect("ESSENTIAL: TaskManager must exist for tests.")
}

/// Pass n_blocks on the parachain-side!
pub fn pass_n(env: &mut TestEnv, n: u64) -> Result<(), ()> {
	for _ in 0..n {
		env.evolve()?;
	}

	Ok(())
}
