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
use std::collections::HashMap;

use cfg_primitives::{AuraId, BlockNumber, Index};
use codec::{Decode, Encode};
use frame_support::traits::GenesisBuild;
use frame_system::EventRecord;
use fudge::{
	digest::{DigestCreator, DigestProvider, FudgeAuraDigest, FudgeBabeDigest},
	inherent::{
		CreateInherentDataProviders, FudgeDummyInherentRelayParachain, FudgeInherentParaParachain,
		FudgeInherentTimestamp,
	},
	primitives::{Chain, PoolState},
	state::StateProvider,
	ParachainBuilder, RelaychainBuilder, TWasmExecutor,
};
use lazy_static::lazy_static;
//pub use macros::{assert_events, events, run};
pub use macros::*;
use polkadot_core_primitives::{Block as RelayBlock, Header as RelayHeader};
use polkadot_parachain::primitives::Id as ParaId;
use sc_executor::{WasmExecutionMethod, WasmExecutor};
use sc_service::{TFullBackend, TFullClient, TaskManager};
use sp_consensus_babe::digests::CompatibleDigestItem;
use sp_consensus_slots::SlotDuration;
use sp_core::H256;
use sp_runtime::{
	generic::BlockId,
	traits::{BlakeTwo256, Extrinsic},
	DigestItem, Storage,
};
use tokio::runtime::Handle;

use crate::{
	chain::{
		centrifuge,
		centrifuge::{
			Block as CentrifugeBlock, Runtime, RuntimeApi as CentrifugeRtApi, RuntimeEvent,
			PARA_ID, WASM_BINARY as CentrifugeCode,
		},
		relay,
		relay::{Runtime as RelayRt, RuntimeApi as RelayRtApi, WASM_BINARY as RelayCode},
	},
	utils::{
		accounts::{Keyring, NonceManager},
		extrinsics::{xt_centrifuge, xt_relay},
		logs,
		time::START_DATE,
	},
};

pub mod macros {
	/// A macro that evolves the chain until the provided event and pattern are
	/// encountered.
	///
	/// Usage:
	/// ```ignore
	/// env::evolve_until_event!(
	/// 		env,
	/// 		Chain::Para(PARA_ID),
	/// 		RuntimeEvent,
	/// 		max_blocks,
	/// 		RuntimeEvent::LiquidityPoolsGateway(pallet_liquidity_pools_gateway::Event::DomainRouterSet {
	/// 			domain,
	/// 			router,
	/// 		}) if [*domain == test_domain && *router == test_router],
	/// 	);
	/// ```
	macro_rules! evolve_until_event_is_found {
		($env:expr, $chain:expr, $event:ty, $max_count:expr, $pattern:pat_param $(if $extra:tt)?, ) => {{
			use frame_support::assert_ok;
			use frame_system::EventRecord as __hidden_EventRecord;
			use sp_core::H256 as __hidden_H256;
			use codec::Decode as _;

			use crate::utils::env::macros::{extra_counts, extra_guards};

			let mut matched: Vec<$event> = Vec::new();

			for _ in 0..$max_count {
				let latest = $env
						.centrifuge
						.with_state(|| frame_system::Pallet::<Runtime>::block_number())
						.expect("Failed retrieving latest block");

				if latest == 0 {
					$env.evolve().unwrap();
					continue
				}

				let scale_events = $env
					.events($chain, EventRange::One(latest))
					.expect("Failed fetching events");

				let events: Vec<$event> = scale_events
					.into_iter()
					.map(|scale_record| {
						__hidden_EventRecord::<$event, __hidden_H256>::decode(
							&mut scale_record.as_slice(),
						)
						.expect("Decoding from chain data does not fail. qed")
					})
					.map(|record| record.event)
					.collect();

				let matches = |event: &RuntimeEvent| {
					match event {
						$pattern $(if extra_guards!($extra))? => true,
						_ => false
					}
				};

				matched = events.clone();
				matched.retain(|event| matches(event));

				if matched.len() > 0 {
					break
				}

				$env.evolve().unwrap();
			}

			let scale_events = $env.events($chain, EventRange::All).expect("Failed fetching events");
			let events: Vec<$event> = scale_events
				.into_iter()
				.map(|scale_record| __hidden_EventRecord::<$event, __hidden_H256>::decode(&mut scale_record.as_slice())
					.expect("Decoding from chain data does not fail. qed"))
				.map(|record| record.event)
				.collect();

			assert!(
				matched.len() == extra_counts!($pattern $(,$extra)?),
				"events do not match the provided pattern - '{}'.\nMatched events: {:?}\nTotal events: {:?}\n",
				stringify!($pattern $(,$extra)?),
				matched,
				events,
			);
		}};
	}

	/// A macro that helps checking whether a given list of events
	/// has been included in the given range of blocks.
	///
	/// Usage:
	/// ```ignore
	/// env::assert_events!(
	/// 		env, //-> The test environment create with fudge::companion
	/// 		Chain::Para(PARA_ID), //-> The chain where we fetch the events from
	/// 		Event, //-> The event-enum type from the runtime
	/// 		EventRange::All, //-> The range of blocks we check for the events
	/// 		RuntimeEvent::System(frame_system::Event::ExtrinsicFailed{..})
	/// 			if [count 0], // -> Ensures zero occurencies of the given event. Could also ensure n-occurencies
	/// 		RuntimeEvent::PoolSystem(pallet_pool_system::RuntimeEvent::Created(id, ..)) if [id == 0], //-> matches only of the id matches to 0
	/// 		RuntimeEvent::Loans(pallet_loans::Event::PoolInitialised(id)) if [id == 0],
	/// 		RuntimeEvent::Loans(pallet_loans::Event::Created(id, loan, asset))
	/// 			if [id == 0 && loan == InstanceId(1) && asset == Asset(4294967296, InstanceId(1))], //-> matches only of the clause matches
	/// 		RuntimeEvent::Loans(pallet_loans::Event::Priced(id, loan)) if [id == 0 && loan == InstanceId(1)],
	/// 	);
	/// ```
	macro_rules! assert_events {
		($env:expr, $chain:expr, $event:ty, $range:expr, $($pattern:pat_param $(if $extra:tt)? ,)+ ) => {{
			use frame_system::EventRecord as __hidden_EventRecord;
			use crate::utils::env::macros::{extra_guards, extra_counts};
			use sp_core::H256 as __hidden_H256;
			use codec::Decode as _;


			let scale_events = $env.events($chain, $range).expect("Failed fetching events");
			let events: Vec<$event> = scale_events
				.into_iter()
				.map(|scale_record| __hidden_EventRecord::<$event, __hidden_H256>::decode(&mut scale_record.as_slice())
					.expect("Decoding from chain data does not fail. qed"))
				.map(|record| record.event)
				.collect();
			let mut msg = "Failed asserting event clause of: ".to_owned();

			$(
				let matches = |event: &RuntimeEvent| {
					match event {
						$pattern $(if extra_guards!($extra) )? => true,
						_ => false
					}
				};

				let mut matched = events.clone();
				matched.retain(|event| matches(event));
				assert!(
					matched.len() == extra_counts!($pattern $(,$extra)?),
					"events do not match the provided pattern - '{}'.\nMatched events: {:?}\nTotal events: {:?}\n",
					stringify!($pattern $(,$extra)?),
					matched,
					events,
				);
			)+

		}};
	}

	/// DO NOT USE! This macro is solely expected to be used within the macros
	/// modules
	macro_rules! extra_counts {
		($pattern:pat_param) => {
			1
		};
		($pattern:pat_param, [$guard:expr]) => {
			1
		};
		($pattern:pat_param, [$guard:expr, count $count:expr]) => {
			$count
		};
		($pattern:pat_param, [count $count:expr]) => {
			$count
		};
	}

	/// DO NOT USE! This macro is solely expected to be used within the macros
	/// modules
	macro_rules! extra_guards {
		( [count $count:expr] ) => {
			true
		};
		( [$guard:expr] ) => {
			$guard
		};
		( [$guard:expr, count $count:expr] ) => {
			$guard
		};
	}

	/// A macro that helps retrieving specific events with a filter
	/// This is useful as the general interface of the TestEnv return
	/// scale-encoded events.
	///
	/// Returns a vector of the given events.
	///
	/// Usage:
	/// ```ignore
	/// env::events!(
	/// 		env, //-> The test environment create with fudge::companion
	/// 		Chain::Para(PARA_ID), //-> The chain where we fetch the events from
	/// 		Event, //-> The event-enum type from the runtime
	/// 		EventRange::All, //-> The range of blocks we check for the events
	/// 		RuntimeEvent::System(frame_system::Event::ExtrinsicFailed{..}) //-> The list of events that should be matched
	/// 			| RuntimeEvent::PoolSystem(pallet_pool_system::RuntimeEvent::Created(id, ..)) if id == 0 //-> matches only of the id matches to 0
	/// 			| RuntimeEvent::Loans(..)
	/// 	);
	/// ```
	macro_rules! events {
		($env:expr, $chain:expr, $event:ty, $range:expr, $(|)? $( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {{
			use frame_system::EventRecord as __hidden_EventRecord;
			use sp_core::H256 as __hidden_H256;
			use codec::Decode as _;

			let scale_events = $env.events($chain, $range).expect("Failed fetching events");
			let event_records: Vec<__hidden_EventRecord<RuntimeEvent, __hidden_H256>> = scale_events
				.into_iter()
				.map(|scale_record| __hidden_EventRecord::<$event, __hidden_H256>::decode(&mut scale_record.as_slice())
					.expect("Decoding from chain data does not fail. qed"))
				.collect();

			let matches = |event: &RuntimeEvent| {
				match *event {
            		$( $pattern )|+ $( if $guard )? => true,
            		_ => false
				}
			};

			let mut searched_events = Vec::new();
			for record in event_records.clone() {
				if matches(&record.event) {
					searched_events.push(record.event);
				}
			}

			(searched_events, event_records)
		}};
	}

	/// A macro that helps including the given calls into a chain
	/// and to progress a chain until all of them are included
	///
	/// Usage:
	/// ```ignore
	/// env::run!(
	/// 	env, //-> The test environment create with fudge::companion
	/// 	Chain::Para(PARA_ID), //-> The chain, where the calls should be submitted
	/// 	Call, //-> The Call-enum of the chains-runtime
	/// 	ChainState::PoolEmpty, //-> The state the chain should evolve to
	/// 	Keyring::Admin => //-> The executing account of the calls below
	/// 	...,	//-> An infinity list of either Call-Variants or Vec<Call>
	/// 	....; // -> End of calls executed by the previously mentioned account
	///     Keyring::Alice => ANOTHER_CALL_HERE;
	/// );
	/// ```
	macro_rules! run {
		// ($env:expr, $chain:expr, $call:ty, $state:expr, $($sender:expr => $($calls:expr),+);*) => {{
		($env:expr, $chain:expr, $call:ty, $state:expr, $($sender:expr => $($calls:expr$(,)?)+);*) => {{
				use codec::Encode as _;

				trait CallAssimilator {
					fn assimilate(self, calls: &mut Vec<$call>);
				}

				impl CallAssimilator for Vec<$call> {
					fn assimilate(self, calls: &mut Vec<$call>) {
						calls.extend(self);
					}
				}

				impl CallAssimilator for $call {
					fn assimilate(self, calls: &mut Vec<$call>) {
						calls.push(self)
					}
				}

				$(
					let mut calls = Vec::new();
					$(
					  $calls.assimilate(&mut calls);
					)*

					let sign_and_submit_res = $env.batch_sign_and_submit($chain, $sender, calls.into_iter().map(|call| call.encode()).collect());
					assert!(sign_and_submit_res.is_ok());
				)*

				let evolve_res = $env.evolve_till($chain, $state);
				assert!(evolve_res.is_ok())
			}
		};
	}
	// Need to export after definition.
	pub(crate) use assert_events;
	pub(crate) use events;
	pub(crate) use evolve_until_event_is_found;
	pub(crate) use extra_counts;
	pub(crate) use extra_guards;
	pub(crate) use run;
}

lazy_static! {
	pub static ref INSTANCE_COUNTER: Arc<sp_std::sync::atomic::AtomicU64> =
		Arc::new(sp_std::sync::atomic::AtomicU64::new(0));
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
			sp_consensus_aura::inherents::InherentDataProvider,
			FudgeInherentParaParachain,
		),
	>,
>;

/// The type creates digests for the chains.
#[allow(unused)]
type CentrifugeDp = Box<dyn DigestCreator<CentrifugeBlock> + Send + Sync>;

/// The type creates digests for the chains.
#[allow(unused)]
type RelayDp = Box<dyn DigestCreator<RelayBlock> + Send + Sync>;

/// A struct that stores all events that have been generated
/// since we are building blocks locally here.
pub struct EventsStorage {
	pub centrifuge: HashMap<BlockNumber, Vec<EventRecord<RuntimeEvent, H256>>>,
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
	One(BlockNumber),
	Range(BlockNumber, BlockNumber),
	Latest,
}

#[fudge::companion]
pub struct TestEnv {
	#[fudge::relaychain]
	pub relay: RelaychainBuilder<RelayBlock, RelayRtApi, RelayRt, RelayCidp, RelayDp>,
	#[fudge::parachain(PARA_ID)]
	pub centrifuge:
		ParachainBuilder<CentrifugeBlock, CentrifugeRtApi, CentrifugeCidp, CentrifugeDp>,
	nonce_manager: Arc<Mutex<NonceManager>>,
	pub events: Arc<Mutex<EventsStorage>>,
}

pub type Header = cfg_primitives::Header;
pub type Block = cfg_primitives::Block;
pub type UncheckedExtrinsic = centrifuge::UncheckedExtrinsic;

type EnvError = Box<dyn std::error::Error>;

// NOTE: Nonce management is a known issue when interacting with a chain and
// wanting       to submit a lot of extrinsic. This interface eases this issues.
impl TestEnv {
	pub fn events(&self, chain: Chain, range: EventRange) -> Result<Vec<Vec<u8>>, EnvError>
	where
		sp_runtime::generic::Block<Header, UncheckedExtrinsic>: sp_runtime::traits::Block,
	{
		match chain {
			Chain::Relay => {
				let latest = self
					.centrifuge
					.with_state(|| frame_system::Pallet::<Runtime>::block_number())?;

				match range {
					EventRange::Latest => self.events_relay(latest),
					EventRange::All => {
						let mut events = Vec::new();
						// We MUST NOT query events at genesis block, as this triggers
						// a panic. Hence, start at 1.
						for block in 1..latest + 1 {
							events.extend(self.events_relay(block)?)
						}

						Ok(events)
					}
					EventRange::Range(from, to) => {
						let mut events = Vec::new();
						for block in from..to + 1 {
							events.extend(self.events_relay(block)?)
						}

						Ok(events)
					}
					EventRange::One(at) => self.events_relay(at),
				}
			}
			Chain::Para(id) => match id {
				_ if id == PARA_ID => {
					let latest = self
						.centrifuge
						.with_state(|| frame_system::Pallet::<Runtime>::block_number())?;

					match range {
						EventRange::Latest => self.events_centrifuge(latest),
						EventRange::All => {
							let mut events = Vec::new();
							// We MUST NOT query events at genesis block, as this triggers
							// a panic. Hence, start at 1.
							for block in 1..latest + 1 {
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
						EventRange::One(at) => self.events_centrifuge(at),
					}
				}
				_ => Err(EnvError::from("parachain not found")),
			},
		}
	}

	fn events_centrifuge(&self, at: BlockNumber) -> Result<Vec<Vec<u8>>, EnvError> {
		self.centrifuge
			.with_state_at(BlockId::Number(at), || {
				frame_system::Pallet::<centrifuge::Runtime>::events()
			})
			.map_err(|e| e.into())
			.map(|records| records.into_iter().map(|record| record.encode()).collect())
	}

	fn events_relay(&self, at: BlockNumber) -> Result<Vec<Vec<u8>>, EnvError> {
		self.relay
			.with_state_at(BlockId::Number(at), || {
				frame_system::Pallet::<relay::Runtime>::events()
			})
			.map_err(|e| e.into())
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
	/// * should be used when a signed extrinsic is dropped -> nonces are out of
	///   sync
	pub fn clear_nonces(&mut self) {
		self.nonce_manager = Arc::new(Mutex::new(NonceManager::new()));
	}

	/// Signs a given call for the given chain. Should only be used if the
	/// extrinsic really should be submitted afterwards.
	/// **NOTE: This will increase the stored nonce of an account**
	pub fn sign(&mut self, chain: Chain, who: Keyring, call: Vec<u8>) -> Result<Vec<u8>, EnvError> {
		let nonce = self.fetch_add_nonce(chain, who);
		match chain {
			Chain::Relay => {
				Ok(xt_relay(self, who, nonce, Decode::decode(&mut call.as_slice())?)?.encode())
			}
			Chain::Para(id) => match id {
				_ if id == PARA_ID => {
					Ok(
						xt_centrifuge(self, who, nonce, Decode::decode(&mut call.as_slice())?)?
							.encode(),
					)
				}
				_ => Err(EnvError::from("parachain not found")),
			},
		}
	}

	/// Submits a previously signed extrinsics to the pool of the respective
	/// chain.
	pub fn submit(&mut self, chain: Chain, xt: Vec<u8>) -> Result<(), EnvError> {
		self.append_extrinsic(chain, xt).map_err(|e| e.into())
	}

	/// Signs and submits an extrinsic to the given chain. Will take the nonce
	/// for the account from the `NonceManager`.
	pub fn sign_and_submit(
		&mut self,
		chain: Chain,
		who: Keyring,
		call: Vec<u8>,
	) -> Result<(), EnvError> {
		let nonce = self.nonce(chain, who);
		let xt = match chain {
			Chain::Relay => {
				xt_relay(self, who, nonce, Decode::decode(&mut call.as_slice())?)?.encode()
			}
			Chain::Para(id) => match id {
				_ if id == PARA_ID => {
					xt_centrifuge(self, who, nonce, Decode::decode(&mut call.as_slice())?)?.encode()
				}
				_ => return Err(EnvError::from("parachain not found")),
			},
		};

		self.append_extrinsic(chain, xt)?;
		self.incr_nonce(chain, who);
		Ok(())
	}

	/// Signs and submits a batch of extrinsic to the given chain. Will take the
	/// nonce for the account from the `NonceManager`.
	///
	/// Returns early of an extrinsic fails to be submitted.
	pub fn batch_sign_and_submit(
		&mut self,
		chain: Chain,
		who: Keyring,
		calls: Vec<Vec<u8>>,
	) -> Result<(), EnvError> {
		for call in calls {
			self.sign_and_submit(chain, who, call)?;
		}

		Ok(())
	}

	pub fn evolve_till(&mut self, chain: Chain, till_state: ChainState) -> Result<(), EnvError> {
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

	fn evolve_till_pool_xts_centrifuge(&mut self, xts: usize) -> Result<(), EnvError> {
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

	fn evolve_till_pool_xts_relay(&mut self, xts: usize) -> Result<(), EnvError> {
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
pub fn test_env_default(handle: Handle) -> TestEnv {
	test_env(handle, None, None)
}

#[allow(unused)]
pub fn test_env_with_relay_storage(handle: Handle, storage: Storage) -> TestEnv {
	test_env(handle, Some(storage), None)
}

#[allow(unused)]
pub fn test_env_with_centrifuge_storage(handle: Handle, storage: Storage) -> TestEnv {
	test_env(handle, None, Some(storage))
}

#[allow(unused)]
pub fn test_env_with_both_storage(
	handle: Handle,
	relay_storage: Storage,
	centrifuge_storage: Storage,
) -> TestEnv {
	test_env(handle, Some(relay_storage), Some(centrifuge_storage))
}

fn test_env(
	handle: Handle,
	relay_storage: Option<Storage>,
	centrifuge_storage: Option<Storage>,
) -> TestEnv {
	logs::init_logs();

	// Build relay-chain builder
	let relay = {
		sp_tracing::enter_span!(sp_tracing::Level::INFO, "Relay - StartUp");
		let mut state =
			StateProvider::<TFullBackend<centrifuge::Block>, centrifuge::Block>::empty_default(
				Some(RelayCode.expect("Wasm is build. Qed.")),
			)
			.expect("ESSENTIAL: State provider can be created");

		// We need to HostConfiguration and use the default here.
		state.insert_storage(
			polkadot_runtime_parachains::configuration::GenesisConfig::<RelayRt>::default()
				.build_storage()
				.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
		);

		state.insert_storage(
			frame_system::GenesisConfig {
				code: RelayCode.expect("ESSENTIAL: Relay WASM is some.").to_vec(),
			}
			.build_storage::<RelayRt>()
			.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
		);

		if let Some(storage) = relay_storage {
			state.insert_storage(storage);
		}

		let mut init = fudge::initiator::default(handle.clone());
		init.with_genesis(Box::new(state));

		let cidp: fn(Arc<_>) -> RelayCidp = |clone_client: Arc<
			TFullClient<RelayBlock, RelayRtApi, TWasmExecutor>,
		>| {
			let instance_id = FudgeInherentTimestamp::create_instance(
				std::time::Duration::from_secs(6),
				Some(std::time::Duration::from_millis(START_DATE)),
			)
			.expect("ESSENTIAL: Instance ID can be created.");

			Box::new(move |parent: H256, ()| {
				let client = clone_client.clone();
				let parent_header = client
					.header(parent.clone())
					.expect("ESSENTIAL: Relay CIDP must not fail.")
					.expect("ESSENTIAL: Relay CIDP must not fail.");

				async move {
					let timestamp = FudgeInherentTimestamp::get_instance(instance_id)
						.expect("Instances is initialized");

					let slot =
							sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
								timestamp.current_time(),
								SlotDuration::from_millis(std::time::Duration::from_secs(6).as_millis() as u64),
							);

					let relay_para_inherent = FudgeDummyInherentRelayParachain::new(parent_header);
					Ok((timestamp, slot, relay_para_inherent))
				}
			})
		};

		let dp: RelayDp = Box::new(
			move |parent: sp_runtime::generic::Header<u32, BlakeTwo256>, inherents| async move {
				let babe = FudgeBabeDigest::<RelayBlock>::new();
				let digest = babe.build_digest(parent, &inherents).await?;
				Ok(digest)
			},
		);

		RelaychainBuilder::<_, _, RelayRt, RelayCidp, RelayDp>::new(init, |client| {
			(cidp(client), dp)
		})
		.expect("ESSENTIAL: Relay chain builder can be created.")
	};

	// Build parachain-builder
	let centrifuge = {
		sp_tracing::enter_span!(sp_tracing::Level::INFO, "Centrifuge - StartUp");
		let mut state =
			StateProvider::<TFullBackend<centrifuge::Block>, centrifuge::Block>::empty_default(
				Some(CentrifugeCode.expect("Wasm is build. Qed.")),
			)
			.expect("ESSENTIAL: State provider can be created.");

		state.insert_storage(
			frame_system::GenesisConfig {
				code: CentrifugeCode
					.expect("ESSENTIAL: Centrifuge WASM is some.")
					.to_vec(),
			}
			.build_storage::<Runtime>()
			.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
		);
		state.insert_storage(
			pallet_aura::GenesisConfig::<Runtime> {
				authorities: vec![AuraId::from(sp_core::sr25519::Public([0u8; 32]))],
			}
			.build_storage()
			.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
		);

		if let Some(storage) = centrifuge_storage {
			state.insert_storage(storage);
		}

		let mut init = fudge::initiator::default(handle);
		init.with_genesis(Box::new(state));

		let para_id = ParaId::from(PARA_ID);
		let inherent_builder = relay.inherent_builder(para_id.clone());
		let instance_id = FudgeInherentTimestamp::create_instance(
			std::time::Duration::from_secs(12),
			Some(std::time::Duration::from_millis(START_DATE)),
		)
		.expect("ESSENTIAL: Instance ID can be created.");

		let cidp = Box::new(move |_parent: H256, ()| {
			let inherent_builder_clone = inherent_builder.clone();
			async move {
				let timestamp = FudgeInherentTimestamp::get_instance(instance_id)
					.expect("Instances is initialized");

				let slot =
					sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
						timestamp.current_time(),
						SlotDuration::from_millis(std::time::Duration::from_secs(12).as_millis() as u64),
					);
				let inherent = inherent_builder_clone.parachain_inherent().await.unwrap();
				let relay_para_inherent = FudgeInherentParaParachain::new(inherent);
				Ok((timestamp, slot, relay_para_inherent))
			}
		});
		let dp = |clone_client: Arc<
			sc_service::TFullClient<CentrifugeBlock, CentrifugeRtApi, TWasmExecutor>,
		>| {
			Box::new(move |parent, inherents| {
				let client = clone_client.clone();

				async move {
					let aura = FudgeAuraDigest::<
						CentrifugeBlock,
						sc_service::TFullClient<CentrifugeBlock, CentrifugeRtApi, TWasmExecutor>,
					>::new(&*client)
					.expect("ESSENTIAL: Aura digest can be created.");

					let digest = aura.build_digest(parent, &inherents).await?;
					Ok(digest)
				}
			})
		};

		ParachainBuilder::<_, _, CentrifugeCidp, CentrifugeDp>::new(init, |client| {
			(cidp, dp(client))
		})
		.expect("ESSENTIAL: Parachain builder can be created.")
	};

	TestEnv::new(
		relay,
		centrifuge,
		Arc::new(Mutex::new(NonceManager::new())),
		Arc::new(Mutex::new(EventsStorage::new())),
	)
	.expect("ESSENTIAL: Creating new TestEnv instance must not fail.")
}

/// Pass n_blocks on the parachain-side!
pub fn pass_n(env: &mut TestEnv, n: u64) -> Result<(), EnvError> {
	for _ in 0..n {
		env.evolve()?;
	}

	Ok(())
}

mod tests {
	use super::*;

	#[tokio::test]
	async fn env_works() {
		let mut env = test_env_default(Handle::current());

		// FIXME: https://github.com/centrifuge/centrifuge-chain/issues/1219
		// Breaks on >= 10 for fast-runtime since session length is 5 blocks
		#[cfg(feature = "fast-runtime")]
		let num_blocks = 9;
		#[cfg(not(feature = "fast-runtime"))]
		let num_blocks = 10;
		let block_before = env
			.with_state(Chain::Para(PARA_ID), || {
				frame_system::Pallet::<Runtime>::block_number()
			})
			.expect("Cannot create block before");

		frame_support::assert_ok!(pass_n(&mut env, num_blocks));

		let block_after = env
			.with_state(Chain::Para(PARA_ID), || {
				frame_system::Pallet::<Runtime>::block_number()
			})
			.expect("Cannot create block after");

		assert_eq!(block_before + num_blocks as u32, block_after)
	}
}
