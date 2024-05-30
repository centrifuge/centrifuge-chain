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

//! Centrifuge RPC endpoints (common endpoints across all environments)

use std::sync::Arc;

use cfg_primitives::{AccountId, Balance, Block, BlockNumber, Hash, Nonce};
use jsonrpsee::types::error::{ErrorCode, ErrorObject};
use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
use runtime_common::apis::AnchorApi;
use sc_rpc_api::DenyUnsafe;
use sc_service::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use substrate_frame_rpc_system::{System, SystemApiServer};

use crate::rpc::anchors::{AnchorApiServer, Anchors};

pub mod anchors;
pub mod evm;

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpsee::RpcModule<()>;

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P>(
	client: Arc<C>,
	pool: Arc<P>,
	deny_unsafe: DenyUnsafe,
) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
	C: ProvideRuntimeApi<Block>
		+ HeaderBackend<Block>
		+ HeaderMetadata<Block, Error = BlockChainError>
		+ Send
		+ Sync
		+ 'static,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
	C::Api: BlockBuilder<Block>,
	C::Api: AnchorApi<Block, Hash, BlockNumber>,
	P: TransactionPool + Sync + Send + 'static,
{
	let mut module = RpcExtension::new(());

	module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
	module.merge(TransactionPayment::new(client.clone()).into_rpc())?;
	module.merge(Anchors::new(client.clone()).into_rpc())?;

	Ok(module)
}

pub fn invalid_params_error(msg: &'static str) -> ErrorObject {
	ErrorObject::owned(ErrorCode::InvalidParams.code(), msg, Option::<()>::None)
}
