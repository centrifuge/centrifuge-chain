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
use cfg_primitives::{
	parachains,
	types::{EnsureRootOr, HalfOfCouncil},
};
use cfg_traits::TryConvert;
use cfg_types::{tokens::CurrencyId, EVMChainId};
use frame_support::{
	parameter_types,
	traits::{Everything, Get, Nothing},
};
use frame_system::EnsureRoot;
use orml_asset_registry::{AssetRegistryTrader, FixedRateAssetRegistryTrader};
use orml_traits::{location::AbsoluteReserveProvider, parameter_type_with_key};
use orml_xcm_support::MultiNativeAsset;
use pallet_xcm::XcmPassthrough;
use runtime_common::{
	transfer_filter::PreXcmTransfer,
	xcm::{
		general_key, AccountIdToMultiLocation, Barrier, FixedConversionRateProvider,
		LpInstanceRelayer, ToTreasury,
	},
	xcm_fees::native_per_second,
};
use sp_core::ConstU32;
use xcm::{latest::Weight as XcmWeight, prelude::*, v3::MultiLocation};
use xcm_builder::{
	ConvertedConcreteId, EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, FungiblesAdapter,
	NoChecking, RelayChainAsNative, SiblingParachainAsNative, SignedAccountId32AsNative,
	SignedToAccountId32, SovereignSignedViaLocation,
};
use xcm_executor::{traits::JustTry, XcmExecutor};

use super::{
	AccountId, Balance, OrmlAssetRegistry, ParachainInfo, ParachainSystem, PolkadotXcm, Runtime,
	RuntimeCall, RuntimeEvent, RuntimeOrigin, Tokens, XcmpQueue,
};

/// A call filter for the XCM Transact instruction. This is a temporary
/// measure until we properly account for proof size weights.
///
/// Calls that are allowed through this filter must:
/// 1. Have a fixed weight;
/// 2. Cannot lead to another call being made;
/// 3. Have a defined proof size weight, e.g. no unbounded vecs in call
/// parameters.
///
/// NOTE: Defensive configuration for now, inspired by filter of
/// SystemParachains and Polkadot, can be extended if desired.
pub struct SafeCallFilter;
impl frame_support::traits::Contains<RuntimeCall> for SafeCallFilter {
	fn contains(call: &RuntimeCall) -> bool {
		matches!(
			call,
			RuntimeCall::Timestamp(..)
				| RuntimeCall::Balances(..)
				| RuntimeCall::Utility(pallet_utility::Call::as_derivative { .. })
				| RuntimeCall::PolkadotXcm(
					pallet_xcm::Call::limited_reserve_transfer_assets { .. }
				) | RuntimeCall::XcmpQueue(..)
				| RuntimeCall::DmpQueue(..)
				| RuntimeCall::Proxy(..)
				| RuntimeCall::LiquidityPoolsGateway(
					pallet_liquidity_pools_gateway::Call::process_msg { .. }
				) | RuntimeCall::OrderBook(..)
		)
	}
}

/// The main XCM config
/// This is where we configure the core of our XCM integrations: how tokens are
/// transferred, how fees are calculated, what barriers we impose on incoming
/// XCM messages, etc.
pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type AssetClaims = PolkadotXcm;
	type AssetExchanger = ();
	type AssetLocker = ();
	// How to withdraw and deposit an asset.
	type AssetTransactor = FungiblesTransactor;
	type AssetTrap = PolkadotXcm;
	type Barrier = Barrier<PolkadotXcm>;
	type CallDispatcher = RuntimeCall;
	type FeeManager = ();
	type IsReserve = MultiNativeAsset<AbsoluteReserveProvider>;
	type IsTeleporter = ();
	type MaxAssetsIntoHolding = ConstU32<64>;
	type MessageExporter = ();
	type OriginConverter = XcmOriginToTransactDispatchOrigin;
	type PalletInstancesInfo = crate::AllPalletsWithSystem;
	type ResponseHandler = PolkadotXcm;
	type RuntimeCall = RuntimeCall;
	type SafeCallFilter = SafeCallFilter;
	type SubscriptionService = PolkadotXcm;
	type Trader = Trader;
	type UniversalAliases = Nothing;
	type UniversalLocation = UniversalLocation;
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	type XcmSender = XcmRouter;
}

/// Trader - The means of purchasing weight credit for XCM execution.
/// We need to ensure we have at least one rule per token we want to handle or
/// else the xcm executor won't know how to charge fees for a transfer of said
/// token.
pub type Trader = (
	FixedRateOfFungible<CanonicalNativePerSecond, ToTreasury<Runtime>>,
	AssetRegistryTrader<
		FixedRateAssetRegistryTrader<FixedConversionRateProvider<OrmlAssetRegistry>>,
		ToTreasury<Runtime>,
	>,
);

parameter_types! {
	// Canonical location: https://github.com/paritytech/polkadot/pull/4470
	pub CanonicalNativePerSecond: (AssetId, u128, u128) = (
		MultiLocation::new(
			0,
			X1(general_key(parachains::kusama::altair::AIR_KEY)),
		).into(),
		native_per_second(),
		0,
	);

}

/// Means for transacting the fungibles assets of this parachain.
pub type FungiblesTransactor = FungiblesAdapter<
	// Use this fungibles implementation
	Tokens,
	// This means that this adapter should handle any token that `CurrencyIdConvert` can convert
	// to `CurrencyId`, the `CurrencyId` type of `Tokens`, the fungibles implementation it uses.
	ConvertedConcreteId<CurrencyId, Balance, CurrencyIdConvert, JustTry>,
	// Convert an XCM MultiLocation into a local account id
	LocationToAccountId,
	// Our chain's account ID type (we can't get away without mentioning it explicitly)
	AccountId,
	// We dont want to allow teleporting assets
	NoChecking,
	// We don't support teleports therefore we don't track them
	CheckingAccount,
>;

parameter_types! {
	// One XCM operation is 200_000_000 weight, cross-chain transfer ~= 2x of transfer.
	pub const UnitWeightCost: XcmWeight = XcmWeight::from_parts(200_000_000, 0);
	pub const MaxInstructions: u32 = 100;
}

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub ReachableDest: Option<MultiLocation> = Some(Parent.into());
}

/// Pallet Xcm offers a lot of out-of-the-box functionality and features to
/// configure and handle XCM messages.
impl pallet_xcm::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId>;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type Currency = crate::Balances;
	type CurrencyMatcher = ();
	type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type MaxLockers = ConstU32<8>;
	type MaxRemoteLockConsumers = ConstU32<0>;
	#[cfg(feature = "runtime-benchmarks")]
	type ReachableDest = ReachableDest;
	type RemoteLockConsumerIdentifier = ();
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type SovereignAccountOf = LocationToAccountId;
	type TrustedLockers = ();
	type UniversalLocation = UniversalLocation;
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	type WeightInfo = crate::weights::pallet_xcm::WeightInfo<Runtime>;
	type XcmExecuteFilter = Nothing;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type XcmReserveTransferFilter = Everything;
	type XcmRouter = XcmRouter;
	type XcmTeleportFilter = Everything;

	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
}

parameter_types! {
	pub const RelayNetwork: NetworkId = NetworkId::Rococo;
	pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
	pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
	pub CheckingAccount: AccountId = PolkadotXcm::check_account();
}

pub type CurrencyIdConvert = runtime_common::xcm::CurrencyIdConvert<Runtime>;
pub type LocationToAccountId = runtime_common::xcm::LocationToAccountId<RelayNetwork>;

/// No local origins on this chain are allowed to dispatch XCM sends/executions.
pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution
/// into the right message queues.
pub type XcmRouter = (
	// Use UMP to communicate with the relay chain
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm, ()>,
	// Use XCMP to communicate with sibling parachains
	XcmpQueue,
);

const MOONBASE_ALPHA_PARA_ID: u32 = 1000;
/// https://chainlist.org/chain/1287
const MOONBASE_ALPHA_EVM_ID: u64 = 1287;

/// A constant way of mapping parachain IDs to EVM-chain IDs
pub struct ParaToEvm;
impl TryConvert<cfg_types::ParaId, EVMChainId> for ParaToEvm {
	type Error = cfg_types::ParaId;

	fn try_convert(a: cfg_types::ParaId) -> Result<EVMChainId, cfg_types::ParaId> {
		// NOTE: Currently only supported moonbeam
		match a {
			MOONBASE_ALPHA_PARA_ID => Ok(MOONBASE_ALPHA_EVM_ID),
			_ => Err(a),
		}
	}
}

/// This is the type we use to convert an (incoming) XCM origin into a local
/// `Origin` instance, ready for dispatching a transaction with Xcm's
/// `Transact`. There is an `OriginKind` which can biases the kind of local
/// `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
	// A matcher that catches all Moonbeam relaying contracts to generate the right Origin
	LpInstanceRelayer<ParaToEvm, Runtime>,
	// Sovereign account converter; this attempts to derive an `AccountId` from the origin location
	// using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
	// foreign chains who want to have a local sovereign account on this chain which they control.
	SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
	// Native converter for Relay-chain (Parent) location; will converts to a `Relay` origin when
	// recognized.
	RelayChainAsNative<RelayChainOrigin, RuntimeOrigin>,
	// Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
	// recognized.
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, RuntimeOrigin>,
	// Native signed account converter; this just converts an `AccountId32` origin into a normal
	// `Origin::Signed` origin of the same 32-byte value.
	SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	XcmPassthrough<RuntimeOrigin>,
);

parameter_types! {
	pub const BaseXcmWeight: XcmWeight = XcmWeight::from_parts(100_000_000, 0);
	pub const MaxAssetsForTransfer: usize = 2;
}

parameter_types! {
	/// The `MultiLocation` identifying this very parachain
	pub SelfLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(ParachainInfo::get().into())));
	pub UniversalLocation: InteriorMultiLocation = X2(
		GlobalConsensus(RelayNetwork::get()),
		Parachain(ParachainInfo::parachain_id().into())
	);
}

parameter_type_with_key! {
	pub ParachainMinFee: |_location: MultiLocation| -> Option<u128> {
		None
	};
}

impl orml_xtokens::Config for Runtime {
	type AccountIdToMultiLocation = AccountIdToMultiLocation<AccountId>;
	type Balance = Balance;
	type BaseXcmWeight = BaseXcmWeight;
	type CurrencyId = CurrencyId;
	type CurrencyIdConvert = CurrencyIdConvert;
	type MaxAssetsForTransfer = MaxAssetsForTransfer;
	type MinXcmFee = ParachainMinFee;
	type MultiLocationsFilter = Everything;
	type ReserveProvider = AbsoluteReserveProvider;
	type RuntimeEvent = RuntimeEvent;
	type SelfLocation = SelfLocation;
	type UniversalLocation = UniversalLocation;
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl pallet_restricted_xtokens::Config for Runtime {
	type PreTransfer = PreXcmTransfer<super::TransferAllowList, CurrencyIdConvert>;
}

impl cumulus_pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl orml_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SovereignOrigin = EnsureRootOr<HalfOfCouncil>;
}
