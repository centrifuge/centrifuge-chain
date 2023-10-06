pub mod environment;
pub mod envs {
	pub mod fudge_env;
	pub mod runtime_env;
}
pub mod runtime;
pub mod utils {
	pub mod genesis;
}

// Test cases
mod cases {
	mod example;
}

use runtime::{Runtime, RuntimeKind};

macro_rules! impl_config {
	($runtime:ident, $kind:ident) => {
		impl Runtime for $runtime::Runtime {
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
