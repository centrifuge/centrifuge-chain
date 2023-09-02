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

use cfg_primitives::{AccountId, Balance, PoolId, TrancheId};
use cfg_types::{domain_address::Domain, fixed_point::Rate};
use frame_support::parameter_types;
use frame_system::EnsureRoot;

use super::{Runtime, RuntimeEvent, RuntimeOrigin};

parameter_types! {
	pub const MaxIncomingMessageSize: u32 = 1024;
}

impl pallet_liquidity_pools_gateway::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId>;
	type InboundQueue = crate::LiquidityPools;
	type LocalEVMOrigin = pallet_liquidity_pools_gateway::EnsureLocal;
	type MaxIncomingMessageSize = MaxIncomingMessageSize;
	type Message = pallet_liquidity_pools::Message<Domain, PoolId, TrancheId, Balance, Rate>;
	type OriginRecovery = crate::LiquidityPoolsAxelarGateway;
	type Router = liquidity_pools_gateway_routers::DomainRouter<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type WeightInfo = ();
}
