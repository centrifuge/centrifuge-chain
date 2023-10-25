pub mod handle;

use std::collections::HashMap;

use cfg_primitives::{Balance, BlockNumber, Index};
use fudge::primitives::Chain;
use handle::{FudgeHandle, ParachainClient};
use sc_client_api::HeaderBackend;
use sp_api::{ApiRef, ProvideRuntimeApi};
use sp_runtime::{generic::BlockId, DispatchError, DispatchResult, Storage};

use crate::{
	generic::{
		config::Runtime,
		env::{utils, Env},
	},
	utils::accounts::Keyring,
};

/// Trait that represent a runtime with Fudge support
pub trait FudgeSupport: Runtime {
	/// Type to interact with fudge
	type FudgeHandle: FudgeHandle<Self>;
}

/// Evironment that uses fudge to interact with the runtime
pub struct FudgeEnv<T: Runtime + FudgeSupport> {
	handle: T::FudgeHandle,
	nonce_storage: HashMap<Keyring, Index>,
}

impl<T: Runtime + FudgeSupport> Env<T> for FudgeEnv<T> {
	fn from_storage(storage: Storage) -> Self {
		let mut handle = T::FudgeHandle::new(Storage::default(), storage);

		handle.evolve();

		Self {
			handle,
			nonce_storage: HashMap::default(),
		}
	}

	fn submit_now(
		&mut self,
		_who: Keyring,
		_call: impl Into<T::RuntimeCallExt>,
	) -> Result<Balance, DispatchError> {
		unimplemented!("FudgeEnv does not support submit_now() try submit_later()")
	}

	fn submit_later(&mut self, who: Keyring, call: impl Into<T::RuntimeCallExt>) -> DispatchResult {
		let nonce = *self.nonce_storage.entry(who).or_default();

		let extrinsic = self.state(|| utils::create_extrinsic::<T>(who, call, nonce));

		self.handle
			.parachain_mut()
			.append_extrinsic(extrinsic)
			.map(|_| ())
			.map_err(|_| {
				DispatchError::Other("Specific kind of DispatchError not supported by fudge now")
				// More information, issue: https://github.com/centrifuge/fudge/issues/67
			})?;

		self.nonce_storage.insert(who, nonce + 1);

		Ok(())
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

mod tests {
	use cfg_primitives::CFG;

	use super::*;
	use crate::generic::{env::Blocks, utils::genesis::Genesis};

	fn correct_nonce_for_submit_later<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_storage(
			Genesis::default()
				.add(pallet_balances::GenesisConfig::<T> {
					balances: vec![(Keyring::Alice.to_account_id(), 1 * CFG)],
				})
				.storage(),
		);

		env.submit_later(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();

		env.submit_later(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();

		env.pass(Blocks::ByNumber(1));

		env.submit_later(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();
	}

	crate::test_for_runtimes!(all, correct_nonce_for_submit_later);
}
