use frame_support::inherent::InherentData;
use sp_runtime::{traits::Block, ApplyExtrinsicResult};

use crate::generic::env::{Config, RuntimeKind};

macro_rules! impl_config {
	($runtime:ident) => {
		impl Config for $runtime::Runtime {
			type Block = $runtime::Block;
			type RuntimeCallExt = $runtime::RuntimeCall;

			const KIND: RuntimeKind = RuntimeKind::Development;

			fn initialize_block(header: &<Self::Block as Block>::Header) {
				$runtime::Executive::initialize_block(header);
			}

			fn apply_extrinsic(
				extrinsic: <Self::Block as Block>::Extrinsic,
			) -> ApplyExtrinsicResult {
				$runtime::Executive::apply_extrinsic(extrinsic)
			}

			fn finalize_block() -> <Self::Block as Block>::Header {
				$runtime::Executive::finalize_block()
			}
		}
	};
}

impl_config!(development_runtime);
impl_config!(altair_runtime);
impl_config!(centrifuge_runtime);

#[macro_export]
macro_rules! test_with_all_runtimes {
	($name:ident) => {
		mod $name {
			use super::*;

			#[test]
			fn development() {
				$name::<development_runtime::Runtime>()
			}

			#[test]
			fn altair() {
				$name::<altair_runtime::Runtime>();
			}

			#[test]
			fn centrifuge() {
				$name::<centrifuge_runtime::Runtime>();
			}
		}
	};
}
