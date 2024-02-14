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

use frame_support::traits::Get;
use pallet_evm_precompile_balances_erc20::{Erc20BalancesPrecompile, Erc20Metadata};
use pallet_evm_precompile_blake2::Blake2F;
use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
use pallet_evm_precompile_dispatch::Dispatch;
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};
use precompile_utils::precompile_set::*;

pub struct NativeErc20Metadata<Symbol>(PhantomData<Symbol>);
impl<Symbol: Get<&'static str>> Erc20Metadata for NativeErc20Metadata<Symbol> {
	fn name() -> &'static str {
		Symbol::get()
	}

	fn symbol() -> &'static str {
		Symbol::get()
	}

	fn decimals() -> u8 {
		18
	}

	fn is_native_currency() -> bool {
		true
	}
}

type EthereumPrecompilesChecks = (AcceptDelegateCall, CallableByContract, CallableByPrecompile);

// Address numbers linked with:
// - https://docs.moonbeam.network/builders/pallets-precompiles/precompiles/overview/#precompiled-contract-addresses
// - https://github.com/centrifuge/liquidity-pools/blob/release-v1.0/src/gateway/routers/axelar/Forwarder.sol#L29

pub const LP_AXELAR_GATEWAY: u64 = 0x800;

#[precompile_utils::precompile_name_from_address]
pub type RuntimePrecompilesAt<R, Symbol> = (
	// Ethereum precompiles:
	// We allow DELEGATECALL to stay compliant with Ethereum behavior.
	PrecompileAt<AddressU64<0x1>, ECRecover, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<0x2>, Sha256, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<0x3>, Ripemd160, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<0x4>, Identity, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<0x5>, Modexp, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<0x6>, Bn128Add, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<0x7>, Bn128Mul, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<0x8>, Bn128Pairing, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<0x9>, Blake2F, EthereumPrecompilesChecks>,
	// Non-Moonbeam specific nor Ethereum precompiles:
	PrecompileAt<AddressU64<0x400>, Sha3FIPS256, (CallableByContract, CallableByPrecompile)>,
	PrecompileAt<AddressU64<0x401>, Dispatch<R>>,
	PrecompileAt<AddressU64<0x402>, ECRecoverPublicKey, (CallableByContract, CallableByPrecompile)>,
	// Moonbeam specific precompiles:
	PrecompileAt<
		AddressU64<0x802>,
		Erc20BalancesPrecompile<R, NativeErc20Metadata<Symbol>>,
		(CallableByContract, CallableByPrecompile),
	>,
	// Centrifuge specific precompiles:
	PrecompileAt<
		AddressU64<LP_AXELAR_GATEWAY>,
		axelar_gateway_precompile::Pallet<R>,
		CallableByContract,
	>,
);

pub type Precompiles<R, Symbol> = PrecompileSetBuilder<R, RuntimePrecompilesAt<R, Symbol>>;
