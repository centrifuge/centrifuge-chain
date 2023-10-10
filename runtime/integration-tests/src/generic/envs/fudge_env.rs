pub mod handle;

use cfg_primitives::BlockNumber;
use fudge::primitives::Chain;
use handle::{FudgeHandle, ParachainClient};
use sc_client_api::HeaderBackend;
use sp_api::{ApiRef, ProvideRuntimeApi};
use sp_runtime::{generic::BlockId, DispatchResult, Storage};

use crate::{
	generic::{environment::Env, runtime::Runtime},
	utils::accounts::Keyring,
};

/// Trait that represent the entity has Fudge support
pub trait FudgeSupport {
	/// Type to interact with fudge chains
	type FudgeHandle: FudgeHandle;
}

/// Evironment that uses fudge to interact with the runtime
pub struct FudgeEnv<T: Runtime + FudgeSupport> {
	handle: T::FudgeHandle,
}

impl<T: Runtime + FudgeSupport> Env<T> for FudgeEnv<T> {
	fn from_storage(storage: Storage) -> Self {
		Self {
			handle: T::FudgeHandle::build(Storage::default(), storage),
		}
	}

	fn submit(&mut self, _who: Keyring, _call: impl Into<T::RuntimeCall>) -> DispatchResult {
		// Access to the handle to do everything
		todo!()
	}

	fn state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R {
		self.handle
			.with_mut_state(Chain::Para(T::FudgeHandle::PARA_ID), f)
	}

	fn state<R>(&self, f: impl FnOnce() -> R) -> R {
		self.handle
			.with_state(Chain::Para(T::FudgeHandle::PARA_ID), f)
	}

	fn __priv_build_block(&mut self, _i: BlockNumber) {
		self.handle.evolve();
	}
}

type ApiRefOf<'a, T> =
	ApiRef<
		'a,
		<ParachainClient<
			<T as FudgeHandle>::ParachainBlock,
			<T as FudgeHandle>::ParachainConstructApi,
		> as sp_api::ProvideRuntimeApi<<T as FudgeHandle>::ParachainBlock>>::Api,
	>;

impl<T: Runtime + FudgeSupport> FudgeEnv<T> {
	pub fn with_api<F>(&self, exec: F)
	where
		F: FnOnce(
			ApiRefOf<T::FudgeHandle>,
			BlockId<<T::FudgeHandle as FudgeHandle>::ParachainBlock>,
		),
	{
		let client = self.handle.parachain().client();
		let best_hash = client.info().best_hash;
		let api = client.runtime_api();
		let best_hash = BlockId::hash(best_hash);

		exec(api, best_hash);
	}
}
