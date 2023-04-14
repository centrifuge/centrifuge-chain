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

// 1025 is chosen for compatibility with Moonbeam
const DISPATCH_ADDR: H160 = addr(1025);

impl<R> PrecompileSet for CentrifugePrecompiles<R>
where
	R: pallet_evm::Config,
	R::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	<R::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<R::AccountId>>,
{
	fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
		if handle.code_address() == DISPATCH_ADDR {
			Some(pallet_evm_precompile_dispatch::Dispatch::<R>::execute(
				handle,
			))
		} else {
			None
		}
	}

	fn is_precompile(&self, address: H160) -> bool {
		address == DISPATCH_ADDR
	}
}

const fn addr(a: u64) -> H160 {
	let b = a.to_be_bytes();
	H160([
		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
	])
}
