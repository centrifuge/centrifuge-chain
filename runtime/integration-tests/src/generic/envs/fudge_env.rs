pub mod handle;

use cfg_primitives::BlockNumber;
use fudge::primitives::Chain;
use handle::{FudgeHandle, ParachainClient};
use sc_client_api::HeaderBackend;
use sp_api::{ApiRef, ProvideRuntimeApi};
use sp_runtime::{generic::BlockId, traits::Block, DispatchError, DispatchResult, Storage};

use crate::generic::{environment::Env, runtime::Runtime};

/// Trait that represent a runtime with Fudge support
pub trait FudgeSupport: Runtime {
	/// Type to interact with fudge
	type FudgeHandle: FudgeHandle<Self>;
}

/// Evironment that uses fudge to interact with the runtime
pub struct FudgeEnv<T: Runtime + FudgeSupport> {
	handle: T::FudgeHandle,
}

impl<T: Runtime + FudgeSupport> Env<T> for FudgeEnv<T> {
	fn from_storage(storage: Storage) -> Self {
		let mut handle = T::FudgeHandle::build(Storage::default(), storage);

		handle.evolve();

		Self { handle }
	}

	fn state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R {
		self.handle.parachain_mut().with_mut_state(f).unwrap()
	}

	fn state<R>(&self, f: impl FnOnce() -> R) -> R {
		self.handle.parachain().with_state(f).unwrap()
	}

	fn __priv_build_block(&mut self, _i: BlockNumber) {
		self.handle.evolve();
	}

	fn __priv_apply_extrinsic(
		&mut self,
		extrinsic: <T::Block as Block>::Extrinsic,
	) -> DispatchResult {
		self.handle
			.parachain_mut()
			.append_extrinsic(extrinsic)
			.map(|_| ())
			.map_err(|_| {
				// More information, issue: https://github.com/centrifuge/fudge/issues/67
				DispatchError::Other("Specific kind of DispatchError not supported by fudge now")
			})
	}
}

type ApiRefOf<'a, T> = ApiRef<
	'a,
	<ParachainClient<
		<T as Runtime>::Block,
		<<T as FudgeSupport>::FudgeHandle as FudgeHandle<T>>::ParachainConstructApi,
	> as sp_api::ProvideRuntimeApi<<T as Runtime>::Block>>::Api,
>;

/// Specialized fudge methods
impl<T: Runtime + FudgeSupport> FudgeEnv<T> {
	pub fn chain_state_mut<R>(&mut self, chain: Chain, f: impl FnOnce() -> R) -> R {
		self.handle.with_mut_state(chain, f)
	}

	pub fn chain_state<R>(&self, chain: Chain, f: impl FnOnce() -> R) -> R {
		self.handle.with_state(chain, f)
	}

	pub fn with_api<F>(&self, exec: F)
	where
		F: FnOnce(ApiRefOf<T>, BlockId<T::Block>),
	{
		let client = self.handle.parachain().client();
		let best_hash = client.info().best_hash;
		let api = client.runtime_api();
		let best_hash = BlockId::hash(best_hash);

		exec(api, best_hash);
	}
}
