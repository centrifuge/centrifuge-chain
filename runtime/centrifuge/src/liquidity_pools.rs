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
	liquidity_pools::GeneralCurrencyPrefix, AccountId, Balance, EnsureRootOr, PalletIndex, PoolId,
	TrancheId, TwoThirdOfCouncil,
};
use cfg_types::{
	fixed_point::Ratio,
	tokens::{CurrencyId, TrancheCurrency},
};
use frame_support::{parameter_types, traits::PalletInfoAccess};
use pallet_liquidity_pools::hooks::{
	CollectedForeignInvestmentHook, CollectedForeignRedemptionHook, DecreasedForeignInvestOrderHook,
};
use runtime_common::{
	account_conversion::AccountConverter, foreign_investments::IdentityPoolCurrencyConverter,
	gateway::GatewayAccountProvider, liquidity_pools::LiquidityPoolsMessage,
	origin::EnsureAccountOrRootOr,
};
use sp_runtime::traits::One;

use crate::{
	ForeignInvestments, Investments, LiquidityPools, LiquidityPoolsAxelarGateway,
	LiquidityPoolsGateway, LocationToAccountId, OrderBook, OrmlAssetRegistry, Permissions,
	PoolSystem, Runtime, RuntimeEvent, RuntimeOrigin, Timestamp, Tokens, TreasuryAccount,
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
	// To be used if we want to register a particular asset in the chain spec, when running the chain locally.
	pub LiquidityPoolsPalletIndex: PalletIndex = <LiquidityPools as PalletInfoAccess>::index() as u8;
}

impl pallet_liquidity_pools::Config for Runtime {
	// NOTE: No need to adapt that. The Router is an artifact and will be removed
	// with FI PR
	type AdminOrigin = EnsureRootOr<TwoThirdOfCouncil>;
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

parameter_types! {
	// A temporary admin account for the LP logic
	// This is a multi-sig controlled pure proxy on mainnet
	// - address: "4eEqmbQMbFfNUg6bQnqi9zgUvQvSpNbUgstEM64Xq9FW58Xv" (on Centrifuge)
	//             (pub key 0x80339e91a87b9c082705fd1a6d39b3e00b46e445ad8c80c127f6a56941c6aa57)
	//
	// This account is besides Root and 2/3-council able to
	// - add valid relayer contracts
	// - rm valid relayer contracts
	// - add valid LP instance contracts
	// - rm valid LP instance contracts
	// - add conversions from Axelar `sourceChain` strings to `DomainAddress`
	// - set the Axelar gateway contract in the Axelar gateway precompile
	pub LpAdminAccount: AccountId = AccountId::new(hex_literal::hex!("80339e91a87b9c082705fd1a6d39b3e00b46e445ad8c80c127f6a56941c6aa57"));
}

impl pallet_liquidity_pools_gateway::Config for Runtime {
	type AdminOrigin = EnsureAccountOrRootOr<LpAdminAccount, TwoThirdOfCouncil>;
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
