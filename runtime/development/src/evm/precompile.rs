use core::marker::PhantomData;

use codec::Decode;
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use pallet_evm::{Precompile, PrecompileHandle, PrecompileResult, PrecompileSet};
use sp_core::H160;

pub struct CentrifugePrecompiles<R>(PhantomData<R>);

impl<R> CentrifugePrecompiles<R> {
	#[allow(clippy::new_without_default)] // We'll never use Default and can't derive it.
	pub fn new() -> Self {
		Self(Default::default())
	}
}

impl<R> PrecompileSet for CentrifugePrecompiles<R>
where
	R: pallet_evm::Config,
	R::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	<R::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<R::AccountId>>,
{
	fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
		// 1025 is chosen for compatibility with Moonbeam
		if handle.code_address() == addr(1025) {
			Some(pallet_evm_precompile_dispatch::Dispatch::<R>::execute(
				handle,
			))
		} else {
			None
		}
	}

	fn is_precompile(&self, address: H160) -> bool {
		address == addr(1025)
	}
}

fn addr(a: u64) -> H160 {
	H160::from_low_u64_be(a)
}
