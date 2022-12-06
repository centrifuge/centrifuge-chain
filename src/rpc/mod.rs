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

use std::{fmt::Debug, sync::Arc};

use cfg_primitives::{AccountId, Balance, Index};
use jsonrpsee::{
	core::Error as JsonRpseeError,
	types::error::{CallError, ErrorCode, ErrorObject},
};
use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
use sc_rpc_api::DenyUnsafe;
use sc_service::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use substrate_frame_rpc_system::{System, SystemApiServer};

pub mod anchors;
pub mod pools;
pub mod rewards;

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpsee::RpcModule<()>;

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, Block>(
	client: Arc<C>,
	pool: Arc<P>,
	deny_unsafe: DenyUnsafe,
) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
	Block: sp_api::BlockT,
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
	C: Send + Sync + 'static,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: BlockBuilder<Block>,
	P: TransactionPool + Sync + Send + 'static,
{
	let mut module = RpcExtension::new(());

	module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
	module.merge(TransactionPayment::new(client).into_rpc())?;

	Ok(module)
}

/// Our custom error type for RPC server errors
#[repr(i32)]
pub enum CustomServerError {
	/// The call failed on the Runtime level
	RuntimeError = 1,
}

pub fn runtime_error<InnerError: Debug>(
	message: &'static str,
	inner_error: InnerError,
) -> JsonRpseeError {
	JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
		ErrorCode::ServerError(CustomServerError::RuntimeError as i32).code(),
		message,
		Some(format!("{:?}", inner_error)),
	)))
}

pub fn invalid_params_error(msg: &'static str) -> JsonRpseeError {
	JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
		ErrorCode::InvalidParams.code(),
		msg,
		Option::<()>::None,
	)))
}
