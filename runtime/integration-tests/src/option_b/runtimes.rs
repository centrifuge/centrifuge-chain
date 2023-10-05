use frame_support::inherent::InherentData;
use sp_runtime::{traits::Block, ApplyExtrinsicResult};

use super::env::{Config, RuntimeKind};

impl Config for development_runtime::Runtime {
	type Block = development_runtime::Block;
	type Extrinsic = development_runtime::UncheckedExtrinsic;
	type RuntimeCallExt = development_runtime::RuntimeCall;

	const KIND: RuntimeKind = RuntimeKind::Development;

	fn execute_block(block: Self::Block) {
		development_runtime::Executive::execute_block(block);
	}

	fn initialize_block(header: &<Self::Block as Block>::Header) {
		development_runtime::Executive::initialize_block(header);
	}

	fn apply_extrinsic(extrinsic: <Self::Block as Block>::Extrinsic) -> ApplyExtrinsicResult {
		development_runtime::Executive::apply_extrinsic(extrinsic)
	}

	fn finalize_block() -> <Self::Block as Block>::Header {
		development_runtime::Executive::finalize_block()
	}
}
