use polkadot_primitives::{AssignmentId, AuthorityDiscoveryId, ValidatorId};
use sp_core::ByteArray;

/// Implements the `Runtime` trait for a runtime
macro_rules! impl_runtime {
	($runtime_path:ident, $kind:ident) => {
		const _: () = {
			use crate::generic::config::{Runtime, RuntimeKind};

			impl Runtime for $runtime_path::Runtime {
				type Api = Self;
				type BlockExt = $runtime_path::Block;
				type MaxTranchesExt = $runtime_path::MaxTranches;
				type RuntimeCallExt = $runtime_path::RuntimeCall;
				type RuntimeEventExt = $runtime_path::RuntimeEvent;
				type RuntimeOriginExt = $runtime_path::RuntimeOrigin;

				const KIND: RuntimeKind = RuntimeKind::$kind;
			}
		};
	};
}

impl_runtime!(development_runtime, Development);
impl_runtime!(altair_runtime, Altair);
impl_runtime!(centrifuge_runtime, Centrifuge);

/*
/// Implements fudge support for a runtime
macro_rules! impl_fudge_support {
	(
		$fudge_companion_type:ident,
		$relay_path:ident,
		$relay_session_keys:expr,
		$parachain_path:ident,
		$parachain_id:literal,
		$sibling_id:literal
	) => {
		const _: () = {
			use fudge::primitives::{Chain, ParaId};
			use polkadot_core_primitives::Block as RelayBlock;
			use sp_api::ConstructRuntimeApi;
			use sp_runtime::Storage;

			use crate::generic::envs::fudge_env::{
				handle::{
					FudgeHandle, ParachainBuilder, ParachainClient, RelayClient, RelaychainBuilder,
				},
				FudgeSupport,
			};

			#[fudge::companion]
			pub struct $fudge_companion_type {
				#[fudge::relaychain]
				pub relay: RelaychainBuilder<$relay_path::RuntimeApi, $relay_path::Runtime>,

				#[fudge::parachain($parachain_id)]
				pub parachain:
					ParachainBuilder<$parachain_path::Block, $parachain_path::RuntimeApi>,

				#[fudge::parachain($sibling_id)]
				pub sibling: ParachainBuilder<$parachain_path::Block, $parachain_path::RuntimeApi>,
			}

			// Implement for T only one time when fudge::companion
			// supports generic in the struct signature.
			impl FudgeHandle<$parachain_path::Runtime> for $fudge_companion_type {
				type ParachainApi = <$parachain_path::RuntimeApi as ConstructRuntimeApi<
					$parachain_path::Block,
					ParachainClient<$parachain_path::Block, Self::ParachainConstructApi>,
				>>::RuntimeApi;
				type ParachainConstructApi = $parachain_path::RuntimeApi;
				type RelayApi = <$relay_path::RuntimeApi as ConstructRuntimeApi<
					RelayBlock,
					RelayClient<Self::RelayConstructApi>,
				>>::RuntimeApi;
				type RelayConstructApi = $relay_path::RuntimeApi;
				type RelayRuntime = $relay_path::Runtime;

				const PARACHAIN_CODE: Option<&'static [u8]> = $parachain_path::WASM_BINARY;
				const PARA_ID: u32 = $parachain_id;
				const RELAY_CODE: Option<&'static [u8]> = $relay_path::WASM_BINARY;
				const SIBLING_ID: u32 = $sibling_id;

				fn new(
					relay_storage: Storage,
					parachain_storage: Storage,
					sibling_storage: Storage,
				) -> Self {
					let relay = Self::new_relay_builder(relay_storage, $relay_session_keys);
					let parachain = Self::new_parachain_builder(
						ParaId::from(Self::PARA_ID),
						&relay,
						parachain_storage,
					);
					let sibling = Self::new_parachain_builder(
						ParaId::from(Self::SIBLING_ID),
						&relay,
						sibling_storage,
					);

					Self::new(relay, parachain, sibling).unwrap()
				}

				fn relay(&self) -> &RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime> {
					&self.relay
				}

				fn relay_mut(
					&mut self,
				) -> &mut RelaychainBuilder<Self::RelayConstructApi, Self::RelayRuntime> {
					&mut self.relay
				}

				fn parachain(
					&self,
				) -> &ParachainBuilder<$parachain_path::Block, Self::ParachainConstructApi> {
					&self.parachain
				}

				fn parachain_mut(
					&mut self,
				) -> &mut ParachainBuilder<$parachain_path::Block, Self::ParachainConstructApi> {
					&mut self.parachain
				}

				fn sibling(
					&self,
				) -> &ParachainBuilder<$parachain_path::Block, Self::ParachainConstructApi> {
					&self.sibling
				}

				fn sibling_mut(
					&mut self,
				) -> &mut ParachainBuilder<$parachain_path::Block, Self::ParachainConstructApi> {
					&mut self.sibling
				}

				fn append_extrinsic(
					&mut self,
					chain: Chain,
					extrinsic: Vec<u8>,
				) -> Result<(), Box<dyn std::error::Error>> {
					self.append_extrinsic(chain, extrinsic)
						.map_err(|e| e.into())
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

			impl FudgeSupport for $parachain_path::Runtime {
				type FudgeHandle = $fudge_companion_type;
			}
		};
	};
}

impl_fudge_support!(
	FudgeDevelopment,
	rococo_runtime,
	default_rococo_session_keys(),
	development_runtime,
	2000,
	2001
);

impl_fudge_support!(
	FudgeAltair,
	staging_kusama_runtime,
	default_kusama_session_keys(),
	altair_runtime,
	2088,
	2089
);

impl_fudge_support!(
	FudgeCentrifuge,
	polkadot_runtime,
	default_polkadot_session_keys(),
	centrifuge_runtime,
	2031,
	2032
);

pub fn default_rococo_session_keys() -> rococo_runtime::SessionKeys {
	rococo_runtime::SessionKeys {
		grandpa: pallet_grandpa::AuthorityId::from_slice([0u8; 32].as_slice()).unwrap(),
		babe: pallet_babe::AuthorityId::from_slice([0u8; 32].as_slice()).unwrap(),
		im_online: pallet_im_online::sr25519::AuthorityId::from_slice([0u8; 32].as_slice())
			.unwrap(),
		para_validator: ValidatorId::from_slice([0u8; 32].as_slice()).unwrap(),
		para_assignment: AssignmentId::from_slice([0u8; 32].as_slice()).unwrap(),
		authority_discovery: AuthorityDiscoveryId::from_slice([0u8; 32].as_slice()).unwrap(),
		beefy: sp_consensus_beefy::ecdsa_crypto::AuthorityId::from_slice([0u8; 33].as_slice())
			.unwrap(),
	}
}

pub fn default_kusama_session_keys() -> staging_kusama_runtime::SessionKeys {
	staging_kusama_runtime::SessionKeys {
		grandpa: pallet_grandpa::AuthorityId::from_slice([0u8; 32].as_slice()).unwrap(),
		babe: pallet_babe::AuthorityId::from_slice([0u8; 32].as_slice()).unwrap(),
		im_online: pallet_im_online::sr25519::AuthorityId::from_slice([0u8; 32].as_slice())
			.unwrap(),
		para_validator: ValidatorId::from_slice([0u8; 32].as_slice()).unwrap(),
		para_assignment: AssignmentId::from_slice([0u8; 32].as_slice()).unwrap(),
		authority_discovery: AuthorityDiscoveryId::from_slice([0u8; 32].as_slice()).unwrap(),
	}
}

pub fn default_polkadot_session_keys() -> polkadot_runtime::SessionKeys {
	polkadot_runtime::SessionKeys {
		grandpa: pallet_grandpa::AuthorityId::from_slice([0u8; 32].as_slice()).unwrap(),
		babe: pallet_babe::AuthorityId::from_slice([0u8; 32].as_slice()).unwrap(),
		im_online: pallet_im_online::sr25519::AuthorityId::from_slice([0u8; 32].as_slice())
			.unwrap(),
		para_validator: ValidatorId::from_slice([0u8; 32].as_slice()).unwrap(),
		para_assignment: AssignmentId::from_slice([0u8; 32].as_slice()).unwrap(),
		authority_discovery: AuthorityDiscoveryId::from_slice([0u8; 32].as_slice()).unwrap(),
	}
}
*/
