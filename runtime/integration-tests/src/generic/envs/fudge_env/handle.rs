use std::sync::Arc;

use cfg_primitives::{AuraId, BlockNumber};
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
use polkadot_parachain::primitives::Id as ParaId;
use polkadot_primitives::runtime_api::ParachainHost;
use sc_block_builder::BlockBuilderApi;
use sc_client_api::Backend;
use sc_service::{TFullBackend, TFullClient};
use sp_api::{ApiExt, ConstructRuntimeApi};
use sp_consensus_aura::{sr25519::AuthorityId, AuraApi};
use sp_consensus_babe::BabeApi;
use sp_consensus_slots::SlotDuration;
use sp_core::H256;
use sp_runtime::Storage;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use tokio::runtime::Handle;

use crate::{generic::runtime::Runtime, utils::time::START_DATE};

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
	type RelayRuntime: frame_system::Config<BlockNumber = BlockNumber>
		+ polkadot_runtime_parachains::paras::Config
		+ polkadot_runtime_parachains::session_info::Config
		+ polkadot_runtime_parachains::initializer::Config;

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
		+ TaggedTransactionQueue<T::Block>;

	const RELAY_CODE: Option<&'static [u8]>;
	const PARACHAIN_CODE: Option<&'static [u8]>;
	const PARA_ID: u32;

	fn relay(&self) -> &RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime>;
	fn relay_mut(&mut self) -> &mut RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime>;

	fn parachain(&self) -> &ParachainBuilder<T::Block, Self::ParachainConstructApi>;
	fn parachain_mut(&mut self) -> &mut ParachainBuilder<T::Block, Self::ParachainConstructApi>;

	fn append_extrinsic(&mut self, chain: Chain, extrinsic: Vec<u8>) -> Result<(), ()>;

	fn with_state<R>(&self, chain: Chain, f: impl FnOnce() -> R) -> R;
	fn with_mut_state<R>(&mut self, chain: Chain, f: impl FnOnce() -> R) -> R;
	fn evolve(&mut self);

	fn build(relay_storage: Storage, parachain_storage: Storage) -> Self;

	fn build_relay(
		storage: Storage,
	) -> RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime> {
		sp_tracing::enter_span!(sp_tracing::Level::INFO, "Relay - StartUp");

		let code = Self::RELAY_CODE.unwrap();
		let mut state = StateProvider::new(code);

		state.insert_storage(
			polkadot_runtime_parachains::configuration::GenesisConfig::<Self::RelayRuntime>::default()
				.build_storage()
                .unwrap()
		);

		state.insert_storage(
			frame_system::GenesisConfig {
				code: code.to_vec(),
			}
			.build_storage::<Self::RelayRuntime>()
			.unwrap(),
		);

		state.insert_storage(storage);

		let mut init = fudge::initiator::default(Handle::current());
		init.with_genesis(Box::new(state));

		let cidp = |client: Arc<RelayClient<Self::RelayConstructApi>>| -> RelayInherentCreator {
			let instance_id = FudgeInherentTimestamp::create_instance(
				std::time::Duration::from_secs(6),
				Some(std::time::Duration::from_millis(START_DATE)),
			);

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

		let dp: DigestCreator<RelayBlock> = Box::new(move |parent, inherents| async move {
			let babe = FudgeBabeDigest::<RelayBlock>::new();
			let digest = babe.build_digest(&parent, &inherents).await?;
			Ok(digest)
		});

		RelaychainBuilder::new(init, |client| (cidp(client), dp))
	}

	fn build_parachain(
		relay: &RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime>,
		storage: Storage,
	) -> ParachainBuilder<T::Block, Self::ParachainConstructApi> {
		sp_tracing::enter_span!(sp_tracing::Level::INFO, "Centrifuge - StartUp");

		let code = Self::PARACHAIN_CODE.unwrap();
		let mut state = StateProvider::new(code);

		state.insert_storage(
			frame_system::GenesisConfig {
				code: code.to_vec(),
			}
			.build_storage::<T>()
			.unwrap(),
		);
		state.insert_storage(
			pallet_aura::GenesisConfig::<T> {
				authorities: vec![AuraId::from(sp_core::sr25519::Public([0u8; 32]))],
			}
			.build_storage()
			.unwrap(),
		);

		state.insert_storage(storage);

		let mut init = fudge::initiator::default(Handle::current());
		init.with_genesis(Box::new(state));

		let para_id = ParaId::from(Self::PARA_ID);
		let inherent_builder = relay.inherent_builder(para_id.clone());
		let instance_id = FudgeInherentTimestamp::create_instance(
			std::time::Duration::from_secs(12),
			Some(std::time::Duration::from_millis(START_DATE)),
		);

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
			Box::new(move |parent, inherents| {
				let client = clone_client.clone();

				async move {
					let aura = FudgeAuraDigest::<
						T::Block,
						ParachainClient<T::Block, Self::ParachainConstructApi>,
					>::new(&*client);

					let digest = aura.build_digest(&parent, &inherents).await?;
					Ok(digest)
				}
			})
		};

		ParachainBuilder::new(init, |client| (cidp, dp(client)))
	}
}
