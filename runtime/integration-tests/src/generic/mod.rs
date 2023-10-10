// For generic modules we can reactive the unused warn disabled on lib.rs
#![warn(unused)]
// Allow dead code for utilities not used yet
#![allow(dead_code)]

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
macro_rules! test_for_runtimes {
	( [ $($runtime:ident),* ], $name:ident ) => {
		mod $name {
			use super::*;

            #[allow(unused)]
            use development_runtime as development;

            #[allow(unused)]
            use altair_runtime as altair;

            #[allow(unused)]
            use centrifuge_runtime as centrifuge;

            $(
                #[tokio::test]
                async fn $runtime() {
                    $name::<$runtime::Runtime>()
                }
            )*
		}
	};
	( all , $name:ident ) => {
		$crate::test_for_runtimes!([development, altair, centrifuge], $name);
    };
}

/// TODO generate this for all runtimes with a macro
mod fudge_handles {
	use fudge::primitives::Chain;
	use polkadot_core_primitives::Block as RelayBlock;
	use sp_api::ConstructRuntimeApi;
	use sp_runtime::Storage;

	use crate::generic::envs::fudge_env::{
		handle::{FudgeHandle, ParachainBuilder, ParachainClient, RelayClient, RelaychainBuilder},
		FudgeSupport,
	};

	const DEVELOPMENT_PARA_ID: u32 = 2000;

	type Relaychain = RelaychainBuilder<rococo_runtime::RuntimeApi, rococo_runtime::Runtime>;
	type Parachain = ParachainBuilder<development_runtime::Block, development_runtime::RuntimeApi>;

	#[fudge::companion]
	pub struct DevelopmentFudge {
		#[fudge::relaychain]
		pub relay: Relaychain,

		#[fudge::parachain(DEVELOPMENT_PARA_ID)]
		pub parachain: Parachain,
	}

	// TODO: Implement for T only once when fudge::companion
	// supports generic in the struct signature.
	impl FudgeHandle for DevelopmentFudge {
		type ParachainApi = <development_runtime::RuntimeApi as ConstructRuntimeApi<
			development_runtime::Block,
			ParachainClient<development_runtime::Block, Self::ParachainConstructApi>,
		>>::RuntimeApi;
		type ParachainBlock = development_runtime::Block;
		type ParachainConstructApi = development_runtime::RuntimeApi;
		type ParachainRuntime = development_runtime::Runtime;
		type RelayApi = <rococo_runtime::RuntimeApi as ConstructRuntimeApi<
			RelayBlock,
			RelayClient<Self::RelayConstructApi>,
		>>::RuntimeApi;
		type RelayConstructApi = rococo_runtime::RuntimeApi;
		type RelayRuntime = rococo_runtime::Runtime;

		const PARACHAIN_CODE: Option<&'static [u8]> = development_runtime::WASM_BINARY;
		const PARA_ID: u32 = DEVELOPMENT_PARA_ID;
		const RELAY_CODE: Option<&'static [u8]> = rococo_runtime::WASM_BINARY;

		fn build(relay_storage: Storage, parachain_storage: Storage) -> Self {
			let relay = Self::build_relay(relay_storage);
			let parachain = Self::build_parachain(&relay, parachain_storage);

			Self { relay, parachain }
		}

		fn relay(&self) -> &Relaychain {
			&self.relay
		}

		fn relay_mut(&mut self) -> &mut Relaychain {
			&mut self.relay
		}

		fn parachain(&self) -> &Parachain {
			&self.parachain
		}

		fn parachain_mut(&mut self) -> &mut Parachain {
			&mut self.parachain
		}

		fn append_extrinsic(&mut self, chain: Chain, extrinsic: Vec<u8>) -> Result<(), ()> {
			self.append_extrinsic(chain, extrinsic)
		}

		fn with_state<R>(&self, chain: Chain, f: impl FnOnce() -> R) -> R {
			self.with_state(chain, f).unwrap()
		}

		fn with_mut_state<R>(&mut self, chain: Chain, f: impl FnOnce() -> R) -> R {
			self.with_mut_state(chain, f).unwrap()
		}

		fn evolve(&mut self) {
			self.evolve().unwrap()
		}
	}

	impl FudgeSupport for development_runtime::Runtime {
		type FudgeHandle = DevelopmentFudge;
	}
}
