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
use cfg_traits::connectors::InboundQueue;
use cfg_types::{domain_address::Domain, fixed_point::Rate};
use frame_support::{dispatch::DispatchResult, parameter_types};
use frame_system::EnsureRoot;

use super::{Runtime, RuntimeEvent, RuntimeOrigin};

type ConnectorsMessage = pallet_connectors::Message<Domain, PoolId, TrancheId, Balance, Rate>;

parameter_types! {
	// TODO(cdamian): Double-check these.
	pub const MaxConnectorsPerDomain: u32 = 10;
}

impl pallet_connectors_gateway::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId>;
	type Connectors = DummyInboundQueue;
	type LocalOrigin = pallet_connectors_gateway::EnsureLocal;
	type MaxConnectorsPerDomain = MaxConnectorsPerDomain;
	type Message = ConnectorsMessage;
	type Router = connectors_gateway_routers::DomainRouter<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type WeightInfo = ();
}

// TODO(cdamian): Implement this for the connectors pallet.
pub struct DummyInboundQueue {}

impl InboundQueue for DummyInboundQueue {
	type Message = ConnectorsMessage;
	type Sender = Domain;

	fn submit(_sender: Self::Sender, _msg: Self::Message) -> DispatchResult {
		Ok(())
	}
}
