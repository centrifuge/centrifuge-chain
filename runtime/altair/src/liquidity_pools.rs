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

use cfg_primitives::{
	liquidity_pools::GeneralCurrencyPrefix, AccountId, Balance, PalletIndex, PoolId, TrancheId,
};
use cfg_types::{
	fixed_point::Ratio,
	tokens::{CurrencyId, TrancheCurrency},
};
use frame_support::{parameter_types, traits::PalletInfoAccess};
use frame_system::EnsureRoot;
use pallet_liquidity_pools::hooks::{
	CollectedForeignInvestmentHook, CollectedForeignRedemptionHook, DecreasedForeignInvestOrderHook,
};
use runtime_common::{
	account_conversion::AccountConverter, foreign_investments::IdentityPoolCurrencyConverter,
	gateway::GatewayAccountProvider, liquidity_pools::LiquidityPoolsMessage,
};
use sp_runtime::traits::One;

use crate::{
	ForeignInvestments, Investments, LiquidityPools, LiquidityPoolsGateway, LocationToAccountId,
	OrderBook, OrmlAssetRegistry, Permissions, PoolSystem, Runtime, RuntimeEvent, RuntimeOrigin,
	Timestamp, Tokens, TreasuryAccount,
};

parameter_types! {
	pub DefaultTokenSellRatio: Ratio = Ratio::one();
}

impl pallet_foreign_investments::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CollectedForeignInvestmentHook = CollectedForeignInvestmentHook<Runtime>;
	type CollectedForeignRedemptionHook = CollectedForeignRedemptionHook<Runtime>;
	type CurrencyConverter = IdentityPoolCurrencyConverter<OrmlAssetRegistry>;
	type CurrencyId = CurrencyId;
	type DecreasedForeignInvestOrderHook = DecreasedForeignInvestOrderHook<Runtime>;
	type DefaultTokenSellRatio = DefaultTokenSellRatio;
	type Investment = Investments;
	type InvestmentId = TrancheCurrency;
	type PoolId = PoolId;
	type PoolInspect = PoolSystem;
	type RuntimeEvent = RuntimeEvent;
	type TokenSwapOrderId = u64;
	type TokenSwaps = OrderBook;
	type TrancheId = TrancheId;
	type WeightInfo = ();
}

parameter_types! {
	pub LiquidityPoolsPalletIndex: PalletIndex = <LiquidityPools as PalletInfoAccess>::index() as u8;
}

impl pallet_liquidity_pools::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId>;
	type AssetRegistry = OrmlAssetRegistry;
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CurrencyId = CurrencyId;
	type DomainAccountToAccountId = AccountConverter<Runtime, LocationToAccountId>;
	type DomainAddressToAccountId = AccountConverter<Runtime, LocationToAccountId>;
	type ForeignInvestment = ForeignInvestments;
	type GeneralCurrencyPrefix = GeneralCurrencyPrefix;
	type OutboundQueue = LiquidityPoolsGateway;
	type Permission = Permissions;
	type PoolId = PoolId;
	type PoolInspect = PoolSystem;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Tokens = Tokens;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = TrancheId;
	type TrancheTokenPrice = PoolSystem;
	type TreasuryAccount = TreasuryAccount;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxIncomingMessageSize: u32 = 1024;
	pub Sender: AccountId = GatewayAccountProvider::<Runtime, LocationToAccountId>::get_gateway_account();
}

impl pallet_liquidity_pools_gateway::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId>;
	type InboundQueue = crate::LiquidityPools;
	type LocalEVMOrigin = pallet_liquidity_pools_gateway::EnsureLocal;
	type MaxIncomingMessageSize = MaxIncomingMessageSize;
	type Message = LiquidityPoolsMessage;
	type OriginRecovery = crate::LiquidityPoolsAxelarGateway;
	type Router = liquidity_pools_gateway_routers::DomainRouter<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Sender = Sender;
	type WeightInfo = ();
}
