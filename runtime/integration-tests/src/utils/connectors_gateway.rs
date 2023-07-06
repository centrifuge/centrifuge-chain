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
use cfg_traits::connectors::Router;
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Rate,
};
use connectors_gateway_routers::DomainRouter;
use development_runtime::connectors::MaxIncomingMessageSize;
use pallet_connectors::Message;
use pallet_connectors_gateway::Call as ConnectorsGatewayCall;
use sp_core::bounded::BoundedVec;

use crate::chain::centrifuge::{Runtime, RuntimeCall};

pub fn set_domain_router(domain: Domain, router: DomainRouter<Runtime>) -> RuntimeCall {
	RuntimeCall::ConnectorsGateway(ConnectorsGatewayCall::set_domain_router { domain, router })
}

pub fn add_connector(connector: DomainAddress) -> RuntimeCall {
	RuntimeCall::ConnectorsGateway(ConnectorsGatewayCall::add_connector { connector })
}

pub fn remove_connector(connector: DomainAddress) -> RuntimeCall {
	RuntimeCall::ConnectorsGateway(ConnectorsGatewayCall::remove_connector { connector })
}

pub fn process_msg(raw_msg: Vec<u8>) -> RuntimeCall {
	let msg = BoundedVec::<
		u8,
		<Runtime as pallet_connectors_gateway::Config>::MaxIncomingMessageSize,
	>::try_from(raw_msg)
	.unwrap();
	RuntimeCall::ConnectorsGateway(ConnectorsGatewayCall::process_msg { msg })
}
