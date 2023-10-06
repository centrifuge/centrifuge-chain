use frame_support::inherent::InherentData;
use sp_runtime::{traits::Block, ApplyExtrinsicResult};

use crate::generic::env::{Config, RuntimeKind};

macro_rules! impl_config {
	($runtime:ident, $kind:ident) => {
		impl Config for $runtime::Runtime {
			type Block = $runtime::Block;
			type RuntimeCallExt = $runtime::RuntimeCall;
			type RuntimeEventExt = $runtime::RuntimeEvent;

			const KIND: RuntimeKind = RuntimeKind::$kind;
		}
	};
}

impl_config!(development_runtime, Development);
impl_config!(altair_runtime, Altair);
impl_config!(centrifuge_runtime, Centrifuge);

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
