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
	liquidity_pools::GeneralCurrencyPrefix, AccountId, Balance, EnsureRootOr, HalfOfCouncil,
	PalletIndex, PoolId, TrancheId,
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
	transfer_filter::PreLpTransfer,
};

use crate::{
	ForeignInvestments, Investments, LiquidityPools, LiquidityPoolsAxelarGateway,
	LiquidityPoolsGateway, LocationToAccountId, OrderBook, OrmlAssetRegistry, Permissions,
	PoolSystem, Runtime, RuntimeEvent, RuntimeOrigin, Timestamp, Tokens, TransferAllowList,
	TreasuryAccount,
};

impl pallet_foreign_investments::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CollectedForeignInvestmentHook = CollectedForeignInvestmentHook<Runtime>;
	type CollectedForeignRedemptionHook = CollectedForeignRedemptionHook<Runtime>;
	type CurrencyConverter = IdentityPoolCurrencyConverter<OrmlAssetRegistry>;
	type CurrencyId = CurrencyId;
	type DecreasedForeignInvestOrderHook = DecreasedForeignInvestOrderHook<Runtime>;
	type Investment = Investments;
	type InvestmentId = TrancheCurrency;
	type PoolInspect = PoolSystem;
	type SwapId = u64;
	type TokenSwaps = OrderBook;
}

parameter_types! {
	// To be used if we want to register a particular asset in the chain spec, when running the chain locally.
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
	type PreTransferFilter = PreLpTransfer<TransferAllowList>;
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
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type InboundQueue = LiquidityPools;
	type LocalEVMOrigin = pallet_liquidity_pools_gateway::EnsureLocal;
	type MaxIncomingMessageSize = MaxIncomingMessageSize;
	type Message = LiquidityPoolsMessage;
	type OriginRecovery = LiquidityPoolsAxelarGateway;
	type Router = liquidity_pools_gateway_routers::DomainRouter<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Sender = Sender;
	type WeightInfo = ();
}
