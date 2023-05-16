// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

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

// This value is chosen to be identical to what Moonbeam, for best
// interoperability. See
// https://docs.moonbeam.network/builders/pallets-precompiles/precompiles/overview/#precompiled-contract-addresses
// for details on how Moonbeam organizes precompile addresses. We will
// follow the same namespacing.
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

// This is a reimplementation of the upstream u64->H160 conversion
// function, made `const` to make our precompile address `const`s a
// bit cleaner. It can be removed when upstream has a const conversion
// function.
const fn addr(a: u64) -> H160 {
	let b = a.to_be_bytes();
	H160([
		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
	])
}
