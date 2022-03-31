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

use crate::chain::centrifuge::{
	Block as CentrifugeBlock, Origin as CentrifugeOrigin, Runtime as CentrifugeRt,
	RuntimeApi as CentrifugeRtApi,
};
use crate::chain::relay::{Origin as RelayOrigin, Runtime as RelayRt, RuntimeApi as RelayRtApi};
use fudge::{
	digest::DigestCreator,
	inherent::{
		CreateInherentDataProviders, FudgeDummyInherentRelayParachain, FudgeInherentParaParachain,
		FudgeInherentTimestamp,
	},
	EnvProvider, ParachainBuilder, RelaychainBuilder,
};
use polkadot_core_primitives::{Block as RelayBlock, Header as RelayHeader};
use sp_runtime::Storage;

/// The type that CreatesInherentDataProviders for the relay-chain.
/// As a new-type here as otherwise the TestEnv is badly
/// readable.
type RelayCidp = Box<
	dyn CreateInherentDataProviders<
		RelayBlock,
		(),
		InherentDataProviders = (
			FudgeInherentTimestamp,
			sp_consensus_babe::inherents::InherentDataProvider,
			sp_authorship::InherentDataProvider<RelayHeader>,
			FudgeDummyInherentRelayParachain<RelayHeader>,
		),
	>,
>;

/// The type that CreatesInherentDataProviders for the para-chain.
/// As a new-type here as otherwise the TestEnv is badly
/// readable.
type CentrifugeCidp = Box<
	dyn CreateInherentDataProviders<
		CentrifugeBlock,
		(),
		InherentDataProviders = (
			FudgeInherentTimestamp,
			sp_consensus_babe::inherents::InherentDataProvider,
			FudgeInherentParaParachain,
		),
	>,
>;

/// The type creates digests for the chains.
type Dp = Box<dyn DigestCreator + Send + Sync>;

#[fudge::companion]
pub struct TestEnv {
	#[fudge::parachain(2031)]
	centrifuge: ParachainBuilder<CentrifugeBlock, CentrifugeRtApi, CentrifugeCidp, Dp>,
	#[fudge::relaychain]
	relay: RelaychainBuilder<RelayBlock, RelayRtApi, RelayRt, RelayCidp, Dp>,
}

pub fn default_test_env() -> TestEnv {
	todo!()
}

pub fn with_storage_test_env(storage: Storage) -> TestEnv {
	todo!()
}
