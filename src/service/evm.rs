use std::{collections::HashMap, sync::Arc};

use cumulus_client_consensus_common::ParachainBlockImportMarker;
use fc_consensus::FrontierBlockImport;
use fp_rpc::EthereumRuntimeRPCApi;
use sc_client_api::{backend::AuxStore, BlockOf};
use sc_consensus::{
	BlockCheckParams, BlockImport as BlockImportT, BlockImportParams, ImportResult,
};
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{well_known_cache_keys::Id as CacheKeyId, HeaderBackend};
use sp_consensus::Error as ConsensusError;
use sp_runtime::traits::Block as BlockT;

#[derive(Clone)]
pub struct BlockImport<B: BlockT, I: BlockImportT<B>, C>(FrontierBlockImport<B, I, C>);

impl<B, I, C> BlockImport<B, I, C>
where
	B: BlockT,
	I: BlockImportT<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
	I::Error: Into<ConsensusError>,
	C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + BlockOf,
	C::Api: EthereumRuntimeRPCApi<B>,
	C::Api: BlockBuilderApi<B>,
{
	pub fn new(inner: I, client: Arc<C>, backend: Arc<fc_db::Backend<B>>) -> Self {
		Self(FrontierBlockImport::new(inner, client, backend))
	}
}

#[async_trait::async_trait]
impl<B, I, C> BlockImportT<B> for BlockImport<B, I, C>
where
	B: BlockT,
	I: BlockImportT<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
	I::Error: Into<ConsensusError>,
	C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + BlockOf,
	C::Api: EthereumRuntimeRPCApi<B>,
	C::Api: BlockBuilderApi<B>,
{
	type Error = ConsensusError;
	type Transaction = sp_api::TransactionFor<C, B>;

	async fn check_block(
		&mut self,
		block: BlockCheckParams<B>,
	) -> Result<ImportResult, Self::Error> {
		self.0.check_block(block).await
	}

	async fn import_block(
		&mut self,
		block: BlockImportParams<B, Self::Transaction>,
		new_cache: HashMap<CacheKeyId, Vec<u8>>,
	) -> Result<ImportResult, Self::Error> {
		self.0.import_block(block, new_cache).await
	}
}

impl<B: BlockT, I: BlockImportT<B>, C> ParachainBlockImportMarker for BlockImport<B, I, C> {}
