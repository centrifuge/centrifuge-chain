use frame_support::inherent::InherentData;
use sp_runtime::{traits::Block, ApplyExtrinsicResult};

use crate::generic::env::{Config, RuntimeKind};

impl Config for development_runtime::Runtime {
	type Block = development_runtime::Block;
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

#[macro_export]
macro_rules! test_with_all_runtimes {
	($setup:ident, $name:ident) => {
		mod $name {
			use super::*;

			#[test]
			fn development() {
				$setup::<development_runtime::Runtime>()
					.execute_with($name::<development_runtime::Runtime>);
			}

			#[test]
			fn altair() {
				$setup::<altair_runtime::Runtime>().execute_with($name::<altair_runtime::Runtime>);
			}

			#[test]
			fn centrifuge() {
				$setup::<centrifuge_runtime::Runtime>()
					.execute_with($name::<centrifuge_runtime::Runtime>);
			}
		}
	};
}
