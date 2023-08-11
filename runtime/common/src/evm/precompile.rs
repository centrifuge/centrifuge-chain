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
use pallet_evm_precompile_blake2::Blake2F;
use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
use pallet_evm_precompile_dispatch::Dispatch;
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};
use sp_core::H160;

// 0000->1023: Standard Ethereum precompiles
const ECRECOVER_ADDR: Addr = addr(1);
const SHA256_ADDR: Addr = addr(2);
const RIPEMD160_ADDR: Addr = addr(3);
const IDENTITY_ADDR: Addr = addr(4);
const MODEXP_ADDR: Addr = addr(5);
const BN128ADD_ADDR: Addr = addr(6);
const BN128MUL_ADDR: Addr = addr(7);
const BN128PAIRING_ADDR: Addr = addr(8);
const BLAKE2F_ADDR: Addr = addr(9);
// 1024->2047: Nonstandard precompiles shared with other chains (such
// as Moonbeam). See
// https://docs.moonbeam.network/builders/pallets-precompiles/precompiles/overview/#precompiled-contract-addresses
const SHA3FIPS256_ADDR: Addr = addr(1024);
const DISPATCH_ADDR: Addr = addr(1025);
const ECRECOVERPUBLICKEY_ADDR: Addr = addr(1026);
// 2048-XXXX: Nonstandard precompiles that are specific to our chain.
// This section intentionally left blank

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
		match handle.code_address().0 {
			ECRECOVER_ADDR => Some(ECRecover::execute(handle)),
			SHA256_ADDR => Some(Sha256::execute(handle)),
			RIPEMD160_ADDR => Some(Ripemd160::execute(handle)),
			IDENTITY_ADDR => Some(Identity::execute(handle)),
			MODEXP_ADDR => Some(Modexp::execute(handle)),
			BN128ADD_ADDR => Some(Bn128Add::execute(handle)),
			BN128MUL_ADDR => Some(Bn128Mul::execute(handle)),
			BN128PAIRING_ADDR => Some(Bn128Pairing::execute(handle)),
			BLAKE2F_ADDR => Some(Blake2F::execute(handle)),
			SHA3FIPS256_ADDR => Some(Sha3FIPS256::execute(handle)),
			DISPATCH_ADDR => Some(Dispatch::<R>::execute(handle)),
			ECRECOVERPUBLICKEY_ADDR => Some(ECRecoverPublicKey::execute(handle)),
			_ => None,
		}
	}

	fn is_precompile(&self, address: H160) -> bool {
		[
			ECRECOVER_ADDR,
			SHA256_ADDR,
			RIPEMD160_ADDR,
			IDENTITY_ADDR,
			MODEXP_ADDR,
			BN128ADD_ADDR,
			BN128MUL_ADDR,
			BN128PAIRING_ADDR,
			BLAKE2F_ADDR,
			SHA3FIPS256_ADDR,
			DISPATCH_ADDR,
			ECRECOVERPUBLICKEY_ADDR,
		]
		.contains(&address.0)
	}
}

// H160 cannot be used in a match statement due to its hand-rolled
// PartialEq implementation. This just gives a nice name to the
// internal array of bytes that an H160 wraps.
type Addr = [u8; 20];

const fn addr(a: u64) -> Addr {
	let b = a.to_be_bytes();
	[
		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
	]
}
