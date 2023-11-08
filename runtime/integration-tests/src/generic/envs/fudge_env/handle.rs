use std::sync::Arc;

use cfg_primitives::{AuraId, Balance, BlockNumber};
use cumulus_primitives_core::CollectCollationInfo;
use frame_support::traits::GenesisBuild;
use fudge::{
	digest::{DigestCreator as DigestCreatorT, DigestProvider, FudgeAuraDigest, FudgeBabeDigest},
	inherent::{
		CreateInherentDataProviders, FudgeDummyInherentRelayParachain, FudgeInherentParaParachain,
		FudgeInherentTimestamp,
	},
	primitives::Chain,
	state::StateProvider,
	TWasmExecutor,
};
use polkadot_core_primitives::{Block as RelayBlock, Header as RelayHeader};
use polkadot_parachain_primitives::primitives::Id as ParaId;
use polkadot_primitives::runtime_api::ParachainHost;
use polkadot_runtime_parachains::configuration::HostConfiguration;
use sc_block_builder::BlockBuilderApi;
use sc_client_api::Backend;
use sc_service::{TFullBackend, TFullClient};
use sp_api::{ApiExt, ConstructRuntimeApi};
use sp_consensus_aura::{sr25519::AuthorityId, AuraApi};
use sp_consensus_babe::BabeApi;
use sp_consensus_slots::SlotDuration;
use sp_core::{crypto::AccountId32, ByteArray, H256};
use sp_runtime::{traits::AccountIdLookup, Storage};
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use tokio::runtime::Handle;

use crate::{chain::centrifuge::Header, generic::config::Runtime, utils::time::START_DATE};

type InherentCreator<Block, InherentParachain, InherentDataProvider> = Box<
	dyn CreateInherentDataProviders<
		Block,
		(),
		InherentDataProviders = (
			FudgeInherentTimestamp,
			InherentDataProvider,
			InherentParachain,
		),
	>,
>;

pub type RelayInherentCreator = InherentCreator<
	RelayBlock,
	FudgeDummyInherentRelayParachain<RelayHeader>,
	sp_consensus_babe::inherents::InherentDataProvider,
>;

pub type ParachainInherentCreator<Block> = InherentCreator<
	Block,
	FudgeInherentParaParachain,
	sp_consensus_aura::inherents::InherentDataProvider,
>;

pub type DigestCreator<Block> = Box<dyn DigestCreatorT<Block> + Send + Sync>;

pub type RelaychainBuilder<RuntimeApi, Runtime> = fudge::RelaychainBuilder<
	RelayBlock,
	RuntimeApi,
	Runtime,
	RelayInherentCreator,
	DigestCreator<RelayBlock>,
>;

pub type ParachainBuilder<Block, RuntimeApi> = fudge::ParachainBuilder<
	Block,
	RuntimeApi,
	ParachainInherentCreator<Block>,
	DigestCreator<Block>,
>;

pub type RelayClient<ConstructApi> = TFullClient<RelayBlock, ConstructApi, TWasmExecutor>;
pub type ParachainClient<Block, ConstructApi> = TFullClient<Block, ConstructApi, TWasmExecutor>;

pub trait FudgeHandle<T: Runtime> {
	type RelayRuntime: frame_system::Config<
			BlockNumber = BlockNumber,
			AccountId = AccountId32,
			Lookup = AccountIdLookup<AccountId32, ()>,
		> + polkadot_runtime_parachains::paras::Config
		+ polkadot_runtime_parachains::session_info::Config
		+ polkadot_runtime_parachains::initializer::Config
		+ polkadot_runtime_parachains::hrmp::Config
		+ pallet_session::Config<ValidatorId = AccountId32>
		+ pallet_xcm::Config
		+ pallet_balances::Config<Balance = Balance>;

	type RelayConstructApi: ConstructRuntimeApi<
			RelayBlock,
			RelayClient<Self::RelayConstructApi>,
			RuntimeApi = Self::RelayApi,
		> + Send
		+ Sync
		+ 'static;

	type RelayApi: BlockBuilderApi<RelayBlock>
		+ BabeApi<RelayBlock>
		+ ParachainHost<RelayBlock>
		+ ApiExt<RelayBlock, StateBackend = <TFullBackend<RelayBlock> as Backend<RelayBlock>>::State>
		+ TaggedTransactionQueue<RelayBlock>;

	type ParachainConstructApi: ConstructRuntimeApi<
			T::Block,
			ParachainClient<T::Block, Self::ParachainConstructApi>,
			RuntimeApi = Self::ParachainApi,
		> + Send
		+ Sync
		+ 'static;

	type ParachainApi: BlockBuilderApi<T::Block>
		+ ApiExt<T::Block, StateBackend = <TFullBackend<T::Block> as Backend<T::Block>>::State>
		+ AuraApi<T::Block, AuthorityId>
		+ TaggedTransactionQueue<T::Block>
		+ CollectCollationInfo<T::Block>;

	const RELAY_CODE: Option<&'static [u8]>;
	const PARACHAIN_CODE: Option<&'static [u8]>;

	const PARA_ID: u32;
	const SIBLING_ID: u32;

	fn relay(&self) -> &RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime>;
	fn relay_mut(&mut self) -> &mut RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime>;

	fn parachain(&self) -> &ParachainBuilder<T::Block, Self::ParachainConstructApi>;
	fn parachain_mut(&mut self) -> &mut ParachainBuilder<T::Block, Self::ParachainConstructApi>;

	fn sibling(&self) -> &ParachainBuilder<T::Block, Self::ParachainConstructApi>;
	fn sibling_mut(&mut self) -> &mut ParachainBuilder<T::Block, Self::ParachainConstructApi>;

	fn append_extrinsic(
		&mut self,
		chain: Chain,
		extrinsic: Vec<u8>,
	) -> Result<(), Box<dyn std::error::Error>>;

	fn with_state<R>(&self, chain: Chain, f: impl FnOnce() -> R) -> R;
	fn with_mut_state<R>(&mut self, chain: Chain, f: impl FnOnce() -> R) -> R;
	fn evolve(&mut self);

	fn new(relay_storage: Storage, parachain_storage: Storage, sibling_storage: Storage) -> Self;

	fn new_relay_builder(
		storage: Storage,
		session_keys: <Self::RelayRuntime as pallet_session::Config>::Keys,
	) -> RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime> {
		sp_tracing::enter_span!(sp_tracing::Level::INFO, "Relay - StartUp");

		let code = Self::RELAY_CODE.expect("ESSENTIAL: WASM is built.");
		let mut state =
			StateProvider::<TFullBackend<RelayBlock>, RelayBlock>::empty_default(Some(code))
				.expect("ESSENTIAL: State provider can be created.");

		let mut configuration = polkadot_runtime_parachains::configuration::GenesisConfig::<
			Self::RelayRuntime,
		>::default();

		let mut host_config = HostConfiguration::<u32>::default();
		host_config.max_downward_message_size = 1024;
		host_config.hrmp_channel_max_capacity = 100;
		host_config.hrmp_channel_max_message_size = 1024;
		host_config.hrmp_channel_max_total_size = 1024;
		host_config.hrmp_max_parachain_outbound_channels = 10;
		host_config.hrmp_max_parachain_inbound_channels = 10;
		host_config.hrmp_max_message_num_per_candidate = 100;
		host_config.max_upward_queue_count = 10;
		host_config.max_upward_queue_size = 1024;
		host_config.max_upward_message_size = 1024;
		host_config.max_upward_message_num_per_candidate = 100;

		configuration.config = host_config;

		state
			.insert_storage(
				configuration
					.build_storage()
					.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
			)
			.expect("ESSENTIAL: Storage can be inserted");

		state
			.insert_storage(
				frame_system::GenesisConfig {
					code: code.to_vec(),
				}
				.build_storage::<Self::RelayRuntime>()
				.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
			)
			.expect("ESSENTIAL: Storage can be inserted");
		state
			.insert_storage(
				pallet_session::GenesisConfig::<Self::RelayRuntime> {
					keys: vec![(
						AccountId32::from_slice([0u8; 32].as_slice()).unwrap(),
						AccountId32::from_slice([0u8; 32].as_slice()).unwrap(),
						session_keys,
					)],
				}
				.build_storage()
				.unwrap(),
			)
			.unwrap();

		state
			.insert_storage(storage)
			.expect("ESSENTIAL: Storage can be inserted");

		let mut init = fudge::initiator::default(Handle::current());
		init.with_genesis(Box::new(state));

		let cidp = |client: Arc<RelayClient<Self::RelayConstructApi>>| -> RelayInherentCreator {
			let instance_id = FudgeInherentTimestamp::create_instance(
				std::time::Duration::from_secs(6),
				Some(std::time::Duration::from_millis(START_DATE)),
			)
			.expect("ESSENTiAL: Instance can be created.");

			Box::new(move |parent: H256, ()| {
				let client = client.clone();
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

		let dp: DigestCreator<RelayBlock> = Box::new(move |parent: Header, inherents| async move {
			let babe = FudgeBabeDigest::<RelayBlock>::new();
			let digest = babe.build_digest(parent.clone(), &inherents).await?;

			Ok(digest)
		});

		RelaychainBuilder::new(init, |client| (cidp(client), dp))
			.expect("ESSENTIAL: Relaychain Builder can be created.")
	}

	fn new_parachain_builder(
		para_id: ParaId,
		relay: &RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime>,
		storage: Storage,
	) -> ParachainBuilder<T::Block, Self::ParachainConstructApi> {
		sp_tracing::enter_span!(sp_tracing::Level::INFO, "Parachain - StartUp");

		let code = Self::PARACHAIN_CODE.expect("ESSENTIAL: WASM is built.");
		let mut state =
			StateProvider::<TFullBackend<T::Block>, T::Block>::empty_default(Some(code))
				.expect("ESSENTIAL: State provider can be created.");

		state
			.insert_storage(
				frame_system::GenesisConfig {
					code: code.to_vec(),
				}
				.build_storage::<T>()
				.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
			)
			.expect("ESSENTIAL: Storage can be inserted");
		state
			.insert_storage(
				pallet_aura::GenesisConfig::<T> {
					authorities: vec![AuraId::from(sp_core::sr25519::Public([0u8; 32]))],
				}
				.build_storage()
				.expect("ESSENTIAL: GenesisBuild must not fail at this stage."),
			)
			.expect("ESSENTIAL: Storage can be inserted");
		state
			.insert_storage(
				<parachain_info::GenesisConfig as GenesisBuild<T>>::build_storage(
					&parachain_info::GenesisConfig {
						parachain_id: para_id,
					},
				)
				.expect("ESSENTIAL: Parachain Info GenesisBuild must not fail at this stage."),
			)
			.expect("ESSENTIAL: Storage can be inserted");

		state
			.insert_storage(storage)
			.expect("ESSENTIAL: Storage can be inserted");

		let mut init = fudge::initiator::default(Handle::current());
		init.with_genesis(Box::new(state));

		let inherent_builder = relay.inherent_builder(para_id.clone());
		let instance_id = FudgeInherentTimestamp::create_instance(
			std::time::Duration::from_secs(12),
			Some(std::time::Duration::from_millis(START_DATE)),
		)
		.expect("ESSENTIAL: Instance can be created.");

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

		let dp = |clone_client: Arc<ParachainClient<T::Block, Self::ParachainConstructApi>>| {
			Box::new(move |parent: Header, inherents| {
				let client = clone_client.clone();

				async move {
					let aura = FudgeAuraDigest::<
						T::Block,
						ParachainClient<T::Block, Self::ParachainConstructApi>,
					>::new(&*client)
					.expect("ESSENTIAL: Aura digest can be created.");

					let digest = aura.build_digest(parent.clone(), &inherents).await?;

					Ok(digest)
				}
			})
		};

		ParachainBuilder::new(para_id, init, |client| (cidp, dp(client)))
			.expect("ESSENTIAL: Parachain Builder can be created.")
	}
}
