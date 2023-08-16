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
use cfg_traits::liquidity_pools::Router;
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Rate,
};
use development_runtime::liquidity_pools::MaxIncomingMessageSize;
use liquidity_pools_gateway_routers::DomainRouter;
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::Call as LiquidityPoolsGatewayCall;
use sp_core::bounded::BoundedVec;

use crate::chain::centrifuge::{Runtime, RuntimeCall};

pub fn set_domain_router_call(domain: Domain, router: DomainRouter<Runtime>) -> RuntimeCall {
	RuntimeCall::LiquidityPoolsGateway(LiquidityPoolsGatewayCall::set_domain_router {
		domain,
		router,
	})
}

pub fn add_instance_call(instance: DomainAddress) -> RuntimeCall {
	RuntimeCall::LiquidityPoolsGateway(LiquidityPoolsGatewayCall::add_instance { instance })
}

pub fn remove_instance_call(instance: DomainAddress) -> RuntimeCall {
	RuntimeCall::LiquidityPoolsGateway(LiquidityPoolsGatewayCall::remove_instance { instance })
}
