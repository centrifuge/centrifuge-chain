// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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
pub mod centrifuge {
	use fudge::{
		digest::DigestCreator,
		inherent::{
			CreateInherentDataProviders, FudgeInherentParaParachain, FudgeInherentTimestamp,
		},
	};

	#[cfg(not(feature = "runtime-benchmarks"))]
	/// HostFunctions that do not include benchmarking specific host functions
	pub type HF = sp_io::SubstrateHostFunctions;

	#[cfg(feature = "runtime-benchmarks")]
	/// Host functions that include benchmarking specific functionalities
	pub type HF = sc_executor::sp_wasm_interface::ExtendedHostFunctions<
		sp_io::SubstrateHostFunctions,
		frame_benchmarking::benchmarking::HostFunctions,
	>;

	/// The type that CreatesInherentDataProviders for the para-chain.
	/// As a new-type here as otherwise the TestEnv is badly
	/// readable.
	pub type Cidp = Box<
		dyn CreateInherentDataProviders<
			Block,
			(),
			InherentDataProviders = (
				FudgeInherentTimestamp,
				sp_consensus_aura::inherents::InherentDataProvider,
				FudgeInherentParaParachain,
			),
		>,
	>;

	/// The type creates digests for the chains.
	pub type Dp = Box<dyn DigestCreator<Block> + Send + Sync>;

	#[cfg(feature = "runtime-altair")]
	pub use altair::*;
	#[cfg(feature = "runtime-centrifuge")]
	pub use centrifuge::*;
	#[cfg(feature = "runtime-development")]
	pub use development::*;

	#[cfg(feature = "runtime-centrifuge")]
	pub mod centrifuge {
		pub use centrifuge_runtime::*;
		pub const PARA_ID: u32 = 2031;
	}

	#[cfg(feature = "runtime-altair")]
	pub mod altair {
		pub use altair_runtime::*;
		pub const PARA_ID: u32 = 2088;
	}

	#[cfg(feature = "runtime-development")]
	pub mod development {
		pub use development_runtime::*;
		pub const PARA_ID: u32 = 2000;
	}
}

pub mod relay {
	use fudge::{
		digest::DigestCreator,
		inherent::{
			CreateInherentDataProviders, FudgeDummyInherentRelayParachain, FudgeInherentTimestamp,
		},
	};

	#[cfg(not(feature = "runtime-benchmarks"))]
	/// HostFunctions that do not include benchmarking specific host functions
	type HF = sp_io::SubstrateHostFunctions;

	#[cfg(feature = "runtime-benchmarks")]
	/// Host functions that include benchmarking specific functionalities
	pub type HF = sc_executor::sp_wasm_interface::ExtendedHostFunctions<
		sp_io::SubstrateHostFunctions,
		frame_benchmarking::benchmarking::HostFunctions,
	>;

	/// The type that CreatesInherentDataProviders for the relay-chain.
	/// As a new-type here as otherwise the TestEnv is badly
	/// readable.
	pub type Cidp = Box<
		dyn CreateInherentDataProviders<
			Block,
			(),
			InherentDataProviders = (
				FudgeInherentTimestamp,
				sp_consensus_babe::inherents::InherentDataProvider,
				FudgeDummyInherentRelayParachain<Header>,
			),
		>,
	>;

	/// The type creates digests for the chains.
	pub type Dp = Box<dyn DigestCreator<Block> + Send + Sync>;

	#[cfg(feature = "runtime-altair")]
	pub use kusama_runtime::*;
	#[cfg(feature = "runtime-centrifuge")]
	pub use polkadot_runtime::*;
	#[cfg(feature = "runtime-development")]
	pub use rococo_runtime::*;
}
