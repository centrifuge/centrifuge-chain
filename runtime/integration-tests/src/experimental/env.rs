use std::{
	collections::HashMap,
	error::Error,
	fmt::Debug,
	sync::{Arc, Mutex},
};

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
use polkadot_core_primitives::Block as RelayBlock;
use polkadot_parachain::primitives::Id as ParaId;
use polkadot_primitives::runtime_api::ParachainHost;
use sc_executor::{WasmExecutionMethod, WasmExecutor};
use sc_service::{TFullClient, TaskManager};
use sp_api::ApiExt;
use sp_block_builder::BlockBuilder;
use sp_consensus_babe::digests::CompatibleDigestItem;
use sp_consensus_slots::SlotDuration;
use sp_core::H256;
use sp_runtime::{
	generic::BlockId,
	traits::{BlakeTwo256, Block, Extrinsic},
	AccountId32, DigestItem, Storage,
};
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use tokio::runtime::Handle;

use crate::utils::{logs, time::START_DATE};

pub trait Config:
	frame_system::Config<AccountId = AccountId32, BlockNumber = BlockNumber>
	+ pallet_pool_system::Config<
		CurrencyId = CurrencyId,
		Balance = Balance,
		PoolId = PoolId,
		TrancheId = TrancheId,
	> + pallet_balances::Config<Balance = Balance>
	+ pallet_investments::Config<InvestmentId = TrancheCurrency, Amount = Balance>
	+ pallet_pool_registry::Config<
		CurrencyId = CurrencyId,
		PoolId = PoolId,
		Balance = Balance,
		ModifyPool = pallet_pool_system::Pallet<Self>,
		ModifyWriteOffPolicy = pallet_loans::Pallet<Self>,
	> + pallet_permissions::Config<Role = Role, Scope = PermissionScope<PoolId, CurrencyId>>
	+ pallet_loans::Config<
		Balance = Balance,
		PoolId = PoolId,
		CollectionId = CollectionId,
		ItemId = ItemId,
	> + orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>
	+ orml_asset_registry::Config<
		AssetId = CurrencyId,
		CustomMetadata = CustomMetadata,
		Balance = Balance,
	> + pallet_uniques::Config<CollectionId = CollectionId, ItemId = ItemId>
	+ pallet_timestamp::Config<Moment = Moment>
	+ pallet_aura::Config<Moment = Moment>
{
	const KIND: RuntimeKind;
}

pub type Header = sp_runtime::generic::Header<BlockNumber, BlakeTwo256>;

type RelayCidp = Box<
	dyn CreateInherentDataProviders<
		RelayBlock,
		(),
		InherentDataProviders = (
			FudgeInherentTimestamp,
			sp_consensus_babe::inherents::InherentDataProvider,
			FudgeDummyInherentRelayParachain<Header>,
		),
	>,
>;

type RelayDp = Box<dyn DigestCreator<RelayBlock> + Send + Sync>;

pub enum EventRange {
	All,
	One(BlockNumber),
	Range(BlockNumber, BlockNumber),
	Latest,
}

pub trait Environment {
	type RelayRuntime: frame_system::Config<BlockNumber = BlockNumber>
		+ polkadot_runtime_parachains::paras::Config
		+ polkadot_runtime_parachains::session_info::Config
		+ polkadot_runtime_parachains::initializer::Config;

	type RelayApi: BlockBuilder<RelayBlock>
		+ ParachainHost<RelayBlock>
		+ ApiExt<RelayBlock>
		+ TaggedTransactionQueue<RelayBlock>
		+ Sync
		+ Send;

	type RelayChainBuilder;

	const RELAY_CODE: Option<&'static [u8]>;

	type ParachainRuntime: Config;
	type ParachainBlock: Block<Header = Header>;
	type ParachainChainBuilder;

	const PARA_ID: u32;

	type BuilderError: Error + Debug;

	fn new_relay(relay_storage: Storage) {
		sp_tracing::enter_span!(sp_tracing::Level::INFO, "Relay - StartUp");
		let relay_code = Self::RELAY_CODE.expect("Wasm is build. Qed.");
		let mut state = StateProvider::new(relay_code);

		state.insert_storage(
			polkadot_runtime_parachains::configuration::GenesisConfig::<Self::RelayRuntime>::default()
				.build_storage()
				.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
		);

		state.insert_storage(
			frame_system::GenesisConfig {
				code: relay_code.to_vec(),
			}
			.build_storage::<Self::RelayRuntime>()
			.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
		);

		state.insert_storage(relay_storage);

		let mut init = fudge::initiator::default::<RelayBlock, Self::RelayApi>(Handle::current());
		init.with_genesis(Box::new(state));

		let cidp: fn(Arc<_>) -> RelayCidp = |clone_client: Arc<
			TFullClient<RelayBlock, Self::RelayApi, TWasmExecutor>,
		>| {
			let instance_id = FudgeInherentTimestamp::create_instance(
				std::time::Duration::from_secs(6),
				Some(std::time::Duration::from_millis(START_DATE)),
			);

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

		let dp: RelayDp = Box::new(move |parent, inherents| async move {
			let babe = FudgeBabeDigest::<RelayBlock>::new();
			let digest = babe.build_digest(&parent, &inherents).await?;
			Ok(digest)
		});

		/*
		RelaychainBuilder::<_, _, RelayRt, RelayCidp, RelayDp>::new(init, |client| {
			(cidp(client), dp)
		})
		*/
	}

	fn relay(&self) -> &Self::RelayChainBuilder;
	fn relay_with_state_at<R>(
		&self,
		at: BlockId<RelayBlock>,
		exec: impl FnOnce() -> R,
	) -> Result<R, Box<dyn std::error::Error>>;

	fn parachain(&self) -> &Self::ParachainChainBuilder;
	fn parachain_with_state_at<R>(
		&self,
		at: BlockId<Self::ParachainBlock>,
		exec: impl FnOnce() -> R,
	) -> Result<R, Box<dyn std::error::Error>>;

	fn append_extrinsics(
		&mut self,
		chain: Chain,
		xts: Vec<Vec<u8>>,
	) -> Result<(), Self::BuilderError> {
		for xt in xts {
			self.append_extrinsic(chain, xt)?;
		}

		Ok(())
	}

	fn append_extrinsic(&mut self, chain: Chain, xt: Vec<u8>) -> Result<(), Self::BuilderError>;

	fn with_state<R>(
		&self,
		chain: Chain,
		exec: impl FnOnce() -> R,
	) -> Result<R, Self::BuilderError>;

	fn with_mut_state<R>(
		&mut self,
		chain: Chain,
		exec: impl FnOnce() -> R,
	) -> Result<R, Self::BuilderError>;

	fn evolve(&mut self) -> Result<(), Self::BuilderError>;

	fn events_relay(&self, at: BlockNumber) -> Result<Vec<Vec<u8>>, ()> {
		self.relay_with_state_at(BlockId::Number(at), || {
			frame_system::Pallet::<Self::RelayRuntime>::events()
		})
		.map_err(|_| ())
		.map(|records| records.into_iter().map(|record| record.encode()).collect())
	}

	fn events_parachain(&self, at: BlockNumber) -> Result<Vec<Vec<u8>>, ()> {
		self.parachain_with_state_at(BlockId::Number(at), || {
			frame_system::Pallet::<Self::ParachainRuntime>::events()
		})
		.map_err(|_| ())
		.map(|records| records.into_iter().map(|record| record.encode()).collect())
	}

	fn events(&self, chain: Chain, range: EventRange) -> Result<Vec<Vec<u8>>, ()> {
		match chain {
			Chain::Relay => {
				let latest = self
					.with_state(chain, || {
						frame_system::Pallet::<Self::RelayRuntime>::block_number()
					})
					.map_err(|_| ())?;

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
				_ if id == Self::PARA_ID => {
					let latest = self
						.with_state(chain, || {
							frame_system::Pallet::<Self::ParachainRuntime>::block_number()
						})
						.map_err(|_| ())?;

					match range {
						EventRange::Latest => self.events_parachain(latest),
						EventRange::All => {
							let mut events = Vec::new();
							// We MUST NOT query events at genesis block, as this triggers
							// a panic. Hence, start at 1.
							for block in 1..latest + 1 {
								events.extend(self.events_parachain(block)?)
							}

							Ok(events)
						}
						EventRange::Range(from, to) => {
							let mut events = Vec::new();
							for block in from..to + 1 {
								events.extend(self.events_parachain(block)?)
							}

							Ok(events)
						}
						EventRange::One(at) => self.events_parachain(at),
					}
				}
				_ => Err(()),
			},
		}
	}
}
