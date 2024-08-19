// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
use cfg_primitives::types::{AccountId, Balance};
use cfg_types::tokens::{CrossChainTransferability, CurrencyId, CustomMetadata};
use frame_support::traits::{fungibles::Mutate, Everything, Get};
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::asset_registry::Inspect;
use polkadot_parachain_primitives::primitives::Sibling;
use sp_runtime::traits::{AccountIdConversion, Convert, MaybeEquivalence, Zero};
use sp_std::marker::PhantomData;
use staging_xcm::v4::{
	Asset, AssetId,
	Fungibility::Fungible,
	Junction::{AccountId32, GeneralKey, Parachain},
	Location, NetworkId,
};
use staging_xcm_builder::{
	AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom,
	AllowTopLevelPaidExecutionFrom, DescribeAllTerminal, DescribeFamily, HashedDescription,
	ParentIsPreset, SiblingParachainConvertsVia, SignedToAccountId32, TakeRevenue,
	TakeWeightCredit,
};

use crate::xcm_fees::{default_per_second, native_per_second};

/// Our FixedConversionRateProvider, used to charge XCM-related fees for
/// tokens registered in the asset registry that were not already handled by
/// native Trader rules.
pub struct FixedConversionRateProvider<OrmlAssetRegistry>(PhantomData<OrmlAssetRegistry>);

impl<
		OrmlAssetRegistry: orml_traits::asset_registry::Inspect<
			AssetId = CurrencyId,
			Balance = Balance,
			CustomMetadata = CustomMetadata,
		>,
	> orml_traits::FixedConversionRateProvider for FixedConversionRateProvider<OrmlAssetRegistry>
{
	fn get_fee_per_second(location: &Location) -> Option<u128> {
		let metadata = OrmlAssetRegistry::metadata_by_location(location)?;
		match metadata.additional.transferability {
			CrossChainTransferability::Xcm(xcm_metadata) => xcm_metadata
				.fee_per_second
				.or_else(|| Some(default_per_second(metadata.decimals))),
			_ => None,
		}
	}
}

/// A utils function to un-bloat and simplify the instantiation of
/// `GeneralKey` values
pub fn general_key(data: &[u8]) -> staging_xcm::latest::Junction {
	GeneralKey {
		length: data.len().min(32) as u8,
		data: cfg_utils::vec_to_fixed_array(data),
	}
}

frame_support::parameter_types! {
	// Canonical location: https://github.com/paritytech/polkadot/pull/4470
	pub CanonicalNativePerSecond: (AssetId, u128, u128) = (
		Location::new(
			0,
			general_key(cfg_primitives::NATIVE_KEY),
		).into(),
		native_per_second(),
		0,
	);
}

/// How we convert an `[AccountId]` into an XCM Location
pub struct AccountIdToLocation;
impl<AccountId: Into<[u8; 32]>> Convert<AccountId, Location> for AccountIdToLocation {
	fn convert(account: AccountId) -> Location {
		AccountId32 {
			network: None,
			id: account.into(),
		}
		.into()
	}
}

/// No local origins on this chain are allowed to dispatch XCM sends/executions.
pub type LocalOriginToLocation<R> = SignedToAccountId32<
	<R as frame_system::Config>::RuntimeOrigin,
	AccountId,
	NetworkIdByGenesis<R>,
>;

pub struct NetworkIdByGenesis<T>(sp_std::marker::PhantomData<T>);

impl<T: frame_system::Config> Get<Option<NetworkId>> for NetworkIdByGenesis<T>
where
	<T as frame_system::Config>::Hash: Into<[u8; 32]>,
{
	fn get() -> Option<NetworkId> {
		Some(NetworkId::ByGenesis(
			frame_system::BlockHash::<T>::get(BlockNumberFor::<T>::zero()).into(),
		))
	}
}

/// CurrencyIdConvert
/// This type implements conversions from our `CurrencyId` type into
/// `Location` and vice-versa. A currency locally is identified with a
/// `CurrencyId` variant but in the network it is identified in the form of a
/// `Location`.
pub struct CurrencyIdConvert<T>(PhantomData<T>);

impl<T> MaybeEquivalence<Location, CurrencyId> for CurrencyIdConvert<T>
where
	T: orml_asset_registry::module::Config<AssetId = CurrencyId, CustomMetadata = CustomMetadata>
		+ staging_parachain_info::Config,
{
	fn convert(location: &Location) -> Option<CurrencyId> {
		let para_id = staging_parachain_info::Pallet::<T>::parachain_id();
		let unanchored_location = match location {
			Location {
				parents: 0,
				interior,
			} => Location {
				parents: 1,
				interior: interior
					.clone()
					.pushed_front_with(Parachain(u32::from(para_id)))
					.ok()?,
			},
			x => x.clone(),
		};

		orml_asset_registry::module::Pallet::<T>::asset_id(&unanchored_location)
	}

	fn convert_back(id: &CurrencyId) -> Option<Location> {
		orml_asset_registry::module::Pallet::<T>::metadata(id)
			.filter(|m| m.additional.transferability.includes_xcm())
			.and_then(|m| m.location)
			.and_then(|l| l.try_into().ok())
	}
}

/// Convert our `CurrencyId` type into its `Location` representation.
/// We use the `AssetRegistry` to lookup the associated `Location` for
/// any given `CurrencyId`, while blocking tokens that are not Xcm-transferable.
impl<T> Convert<CurrencyId, Option<Location>> for CurrencyIdConvert<T>
where
	T: orml_asset_registry::module::Config<AssetId = CurrencyId, CustomMetadata = CustomMetadata>
		+ staging_parachain_info::Config,
{
	fn convert(id: CurrencyId) -> Option<Location> {
		<Self as MaybeEquivalence<_, _>>::convert_back(&id)
	}
}

/// Convert an incoming `Location` into a `CurrencyId` through a
/// reverse-lookup using the AssetRegistry. In the registry, we register CFG
/// using its absolute, non-anchored Location so we need to unanchor the
/// input location for Centrifuge-native assets for that to work.
impl<T> Convert<Location, Option<CurrencyId>> for CurrencyIdConvert<T>
where
	T: orml_asset_registry::module::Config<AssetId = CurrencyId, CustomMetadata = CustomMetadata>
		+ staging_parachain_info::Config,
{
	fn convert(location: Location) -> Option<CurrencyId> {
		<Self as MaybeEquivalence<_, _>>::convert(&location)
	}
}

pub struct ToTreasury<T>(PhantomData<T>);
impl<T> TakeRevenue for ToTreasury<T>
where
	T: orml_asset_registry::module::Config<AssetId = CurrencyId, CustomMetadata = CustomMetadata>
		+ staging_parachain_info::Config
		+ pallet_restricted_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>,
{
	fn take_revenue(revenue: Asset) {
		if let Asset {
			id: AssetId(location),
			fun: Fungible(amount),
		} = revenue
		{
			if let Some(currency_id) =
				<CurrencyIdConvert<T> as MaybeEquivalence<_, _>>::convert(&location)
			{
				let treasury_account = cfg_types::ids::TREASURY_PALLET_ID.into_account_truncating();
				let _ = pallet_restricted_tokens::Pallet::<T>::mint_into(
					currency_id,
					&treasury_account,
					amount,
				);
			}
		}
	}
}

/// Barrier is a filter-like option controlling what messages are allows to be
/// executed.
pub type Barrier<PolkadotXcm> = (
	TakeWeightCredit,
	AllowTopLevelPaidExecutionFrom<Everything>,
	// Expected responses are OK.
	AllowKnownQueryResponses<PolkadotXcm>,
	// Subscriptions for version tracking are OK.
	AllowSubscriptionsFrom<Everything>,
);

/// Type for specifying how a `Location` can be converted into an
/// `AccountId`. This is used when determining ownership of accounts for asset
/// transacting and when attempting to use XCM `Transact` in order to determine
/// the dispatch Origin.
pub type LocationToAccountId<RelayNetwork> = (
	// The parent (Relay-chain) origin converts to the default `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
	// Straight up local `AccountId32` origins just alias directly to `AccountId`.
	AccountId32Aliases<RelayNetwork, AccountId>,
	// Generate remote accounts according to polkadot standards
	HashedDescription<AccountId, DescribeFamily<DescribeAllTerminal>>,
);
