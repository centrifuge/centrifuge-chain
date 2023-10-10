pub mod handle;

use handle::FudgeHandle;
use sp_runtime::{DispatchResult, Storage};

use crate::{
	generic::{
		environment::{Blocks, Env},
		runtime::Runtime,
	},
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

	fn pass(&mut self, _blocks: Blocks<T>) {
		// Access to the handle to do everything
		todo!()
	}

	fn state_mut<R>(&mut self, _f: impl FnOnce() -> R) -> R {
		// Access to the handle to do everything
		todo!()
	}

	fn state<R>(&self, _f: impl FnOnce() -> R) -> R {
		// Access to the handle to do everything
		todo!()
	}
}
