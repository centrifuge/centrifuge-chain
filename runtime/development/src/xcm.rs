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
	constants::currency_decimals,
	parachains,
	types::{EnsureRootOr, HalfOfCouncil},
};
use cfg_traits::TryConvert;
pub use cfg_types::tokens::CurrencyId;
use cfg_types::EVMChainId;
pub use cumulus_primitives_core::ParaId;
pub use frame_support::{
	parameter_types,
	traits::{Contains, Everything, Get, Nothing},
	weights::Weight,
};
use frame_support::{sp_std::marker::PhantomData, traits::fungibles};
use orml_asset_registry::{AssetRegistryTrader, FixedRateAssetRegistryTrader};
use orml_traits::{location::AbsoluteReserveProvider, parameter_type_with_key, MultiCurrency};
use orml_xcm_support::MultiNativeAsset;
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::Sibling;
use runtime_common::{
	xcm::{general_key, AccountIdToMultiLocation, FixedConversionRateProvider, LpGatewayInstance},
	xcm_fees::{default_per_second, ksm_per_second, native_per_second},
};
use sp_core::ConstU32;
use sp_runtime::traits::{Convert, Zero};
use xcm::{latest::Weight as XcmWeight, prelude::*};
use xcm_builder::{
	Account32Hash, AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom,
	AllowTopLevelPaidExecutionFrom, ConvertedConcreteId, EnsureXcmOrigin, FixedRateOfFungible,
	FixedWeightBounds, FungiblesAdapter, NoChecking, ParentIsPreset, RelayChainAsNative,
	SiblingParachainAsNative, SiblingParachainConvertsVia, SignedAccountId32AsNative,
	SignedToAccountId32, SovereignSignedViaLocation, TakeRevenue, TakeWeightCredit,
};
use xcm_executor::{traits::JustTry, XcmExecutor};

use super::{
	AccountId, Balance, OrmlAssetRegistry, OrmlTokens, ParachainInfo, ParachainSystem, PolkadotXcm,
	Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, Tokens, TreasuryAccount, XcmpQueue,
};

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
	type Barrier = Barrier;
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
	type SafeCallFilter = Everything;
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
	FixedRateOfFungible<CanonicalNativePerSecond, ToTreasury>,
	AssetRegistryTrader<
		FixedRateAssetRegistryTrader<FixedConversionRateProvider<OrmlAssetRegistry>>,
		ToTreasury,
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

	pub NativePerSecond: (AssetId, u128) = (
		MultiLocation::new(
			1,
			X2(Parachain(ParachainInfo::parachain_id().into()), general_key(parachains::kusama::altair::AIR_KEY)),
		).into(),
		native_per_second(),
	);

	pub KsmPerSecond: (AssetId, u128) = (MultiLocation::parent().into(), ksm_per_second());

	pub AUSDPerSecond: (AssetId, u128) = (
		MultiLocation::new(
			1,
			X2(
				Parachain(parachains::kusama::karura::ID),
				general_key(parachains::kusama::karura::AUSD_KEY)
			)
		).into(),
		default_per_second(currency_decimals::AUSD)
	);

}

pub struct ToTreasury;
impl TakeRevenue for ToTreasury {
	fn take_revenue(revenue: MultiAsset) {
		use xcm_executor::traits::Convert;

		if let MultiAsset {
			id: Concrete(location),
			fun: Fungible(amount),
		} = revenue
		{
			if let Ok(currency_id) =
				<CurrencyIdConvert as Convert<MultiLocation, CurrencyId>>::convert(location)
			{
				let _ = OrmlTokens::deposit(currency_id, &TreasuryAccount::get(), amount);
			}
		}
	}
}

/// Barrier is a filter-like option controlling what messages are allows to be
/// executed.
pub type Barrier = (
	TakeWeightCredit,
	xcm_primitives::AllowTopLevelPaidExecutionDescendOriginFirst<Everything>,
	AllowTopLevelPaidExecutionFrom<Everything>,
	// Expected responses are OK.
	AllowKnownQueryResponses<PolkadotXcm>,
	// Subscriptions for version tracking are OK.
	AllowSubscriptionsFrom<Everything>,
);

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
	pub const UnitWeightCost: XcmWeight = XcmWeight::from_ref_time(200_000_000);
	pub const MaxInstructions: u32 = 100;
}

/// Allow checking in assets that have issuance > 0.
pub struct NonZeroIssuance<AccountId, Assets>(PhantomData<(AccountId, Assets)>);
impl<AccountId, Assets> Contains<<Assets as fungibles::Inspect<AccountId>>::AssetId>
	for NonZeroIssuance<AccountId, Assets>
where
	Assets: fungibles::Inspect<AccountId>,
{
	fn contains(id: &<Assets as fungibles::Inspect<AccountId>>::AssetId) -> bool {
		!Assets::total_issuance(*id).is_zero()
	}
}

/// CurrencyIdConvert
/// This type implements conversions from our `CurrencyId` type into
/// `MultiLocation` and vice-versa. A currency locally is identified with a
/// `CurrencyId` variant but in the network it is identified in the form of a
/// `MultiLocation`, in this case a pair (Para-Id, Currency-Id).
pub struct CurrencyIdConvert;

/// Convert our `CurrencyId` type into its `MultiLocation` representation.
/// Other chains need to know how this conversion takes place in order to
/// handle it on their side.
impl Convert<CurrencyId, Option<MultiLocation>> for CurrencyIdConvert {
	fn convert(id: CurrencyId) -> Option<MultiLocation> {
		match id {
			CurrencyId::Tranche(_, _) => None,
			_ => OrmlAssetRegistry::multilocation(&id).ok()?,
		}
	}
}

/// Convert an incoming `MultiLocation` into a `CurrencyId` if possible.
/// Here we need to know the canonical representation of all the tokens we
/// handle in order to correctly convert their `MultiLocation` representation
/// into our internal `CurrencyId` type.
impl xcm_executor::traits::Convert<MultiLocation, CurrencyId> for CurrencyIdConvert {
	fn convert(location: MultiLocation) -> Result<CurrencyId, MultiLocation> {
		match location {
			MultiLocation {
				parents: 1,
				interior: X3(Parachain(para_id), PalletInstance(_), GeneralKey { .. }),
			} => match para_id {
				// Note: Until we have pools on Centrifuge, we don't know the pools pallet index
				// and can't therefore match specifically on the Tranche tokens' multilocation;
				// However, we can preemptively assume that any Centrifuge X3-based asset refers
				// to a Tranche token and explicitly fail its conversion to avoid Tranche tokens
				// from being transferred through XCM without permission checks. This is fine since
				// we don't have any other native token represented as an X3 neither do we plan to.
				id if id == u32::from(ParachainInfo::get()) => Err(location),
				// Still support X3-based MultiLocations native to other chains
				_ => OrmlAssetRegistry::location_to_asset_id(location).ok_or(location),
			},
			_ => OrmlAssetRegistry::location_to_asset_id(location).ok_or(location),
		}
	}
}

impl Convert<MultiAsset, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(asset: MultiAsset) -> Option<CurrencyId> {
		if let MultiAsset {
			id: Concrete(location),
			..
		} = asset
		{
			<CurrencyIdConvert as xcm_executor::traits::Convert<_, _>>::convert(location).ok()
		} else {
			None
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub ReachableDest: Option<MultiLocation> = Some(Parent.into());
}

/// Pallet Xcm offers a lot of out-of-the-box functionality and features to
/// configure and handle XCM messages.
impl pallet_xcm::Config for Runtime {
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type Currency = crate::Balances;
	type CurrencyMatcher = ();
	type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type MaxLockers = ConstU32<8>;
	#[cfg(feature = "runtime-benchmarks")]
	type ReachableDest = ReachableDest;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type SovereignAccountOf = ();
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
	pub const KsmLocation: MultiLocation = MultiLocation::parent();
	pub const RelayNetwork: NetworkId = NetworkId::Kusama;
	pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
	pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
	pub CheckingAccount: AccountId = PolkadotXcm::check_account();
}

/// Type for specifying how a `MultiLocation` can be converted into an
/// `AccountId`. This is used when determining ownership of accounts for asset
/// transacting and when attempting to use XCM `Transact` in order to determine
/// the dispatch Origin.
pub type LocationToAccountId = (
	// The parent (Relay-chain) origin converts to the default `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
	// If we receive a MultiLocation of type AccountId32 that is within Centrifuge,
	// just alias it to a local [AccountId].
	AccountId32Aliases<RelayNetwork, AccountId>,
	// A wildcard MultiLocation to AccountId conversion for all the other MultiLocations
	// within the same Relay network.
	Account32Hash<RelayNetwork, AccountId>,
);

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
const MOONBASE_ALPHA_EVM_ID: u64 = 1282;

/// A constant way of mapping parachain IDs to EVM-chain IDs
pub struct ParaToEvm;
impl TryConvert<cfg_types::ParaId, EVMChainId> for ParaToEvm {
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
	LpGatewayInstance<ParaToEvm, Runtime>,
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
	pub const BaseXcmWeight: XcmWeight = XcmWeight::from_ref_time(100_000_000);
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

impl cumulus_pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl orml_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SovereignOrigin = EnsureRootOr<HalfOfCouncil>;
}
