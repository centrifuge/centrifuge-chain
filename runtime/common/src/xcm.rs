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
use cfg_traits::TryConvert;
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	EVMChainId, ParaId,
};
use frame_support::traits::{fungibles::Mutate, Everything, Get};
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::asset_registry::Inspect;
use polkadot_parachain_primitives::primitives::Sibling;
use sp_runtime::traits::{AccountIdConversion, Convert, MaybeEquivalence, Zero};
use sp_std::marker::PhantomData;
use staging_xcm::v4::{
	Asset, AssetId,
	Fungibility::Fungible,
	Junction::{AccountId32, AccountKey20, GeneralKey, Parachain},
	Location, NetworkId, OriginKind,
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

pub struct LpInstanceRelayer<ParaAsEvmChain, Runtime>(PhantomData<(ParaAsEvmChain, Runtime)>);
impl<ParaAsEvmChain, Runtime>
	staging_xcm_executor::traits::ConvertOrigin<<Runtime as frame_system::Config>::RuntimeOrigin>
	for LpInstanceRelayer<ParaAsEvmChain, Runtime>
where
	ParaAsEvmChain: TryConvert<ParaId, EVMChainId>,
	Runtime: pallet_liquidity_pools_gateway::Config,
	<Runtime as frame_system::Config>::RuntimeOrigin:
		From<pallet_liquidity_pools_gateway::GatewayOrigin>,
{
	fn convert_origin(
		origin: impl Into<Location>,
		kind: OriginKind,
	) -> Result<<Runtime as frame_system::Config>::RuntimeOrigin, Location> {
		let location = origin.into();
		match (kind, location.clone().unpack()) {
			(OriginKind::SovereignAccount, (1, [Parachain(para), AccountKey20 { key, .. }])) => {
				let evm_id = ParaAsEvmChain::try_convert(*para).map_err(|_| location.clone())?;
				let domain_address = DomainAddress::EVM(evm_id, *key);

				pallet_liquidity_pools_gateway::Pallet::<Runtime>::relayer(
					Domain::EVM(evm_id),
					&domain_address,
				)
				.ok_or(location.clone())?;

				Ok(
					pallet_liquidity_pools_gateway::GatewayOrigin::AxelarRelay(domain_address)
						.into(),
				)
			}
			_ => Err(location),
		}
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

#[cfg(test)]
mod test {
	use cfg_mocks::{pallet_mock_liquidity_pools, pallet_mock_routers, RouterMock};
	use cfg_primitives::OutboundMessageNonce;
	use cfg_traits::liquidity_pools::test_util::Message;
	use frame_support::{assert_ok, derive_impl, traits::EnsureOrigin};
	use frame_system::EnsureRoot;
	use pallet_liquidity_pools_gateway::{EnsureLocal, GatewayOrigin};
	use sp_core::{ConstU32, ConstU64};
	use sp_runtime::DispatchError;
	use staging_xcm_executor::traits::ConvertOrigin;

	use super::*;

	type AccountId = u64;

	// For testing the pallet, we construct a mock runtime.
	frame_support::construct_runtime!(
		pub enum Runtime {
			System: frame_system,
			Gateway: pallet_liquidity_pools_gateway,
			MockLP: pallet_mock_liquidity_pools,
			MockParaAsEvmChain: cfg_mocks::converter::pallet::<Instance1>,
			MockOriginRecovery: cfg_mocks::converter::pallet::<Instance2>,
		}
	);

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
	impl frame_system::Config for Runtime {
		type Block = frame_system::mocking::MockBlock<Runtime>;
	}

	impl cfg_mocks::converter::pallet::Config<cfg_mocks::converter::pallet::Instance1> for Runtime {
		type From = ParaId;
		type To = EVMChainId;
	}

	impl cfg_mocks::converter::pallet::Config<cfg_mocks::converter::pallet::Instance2> for Runtime {
		type From = (Vec<u8>, Vec<u8>);
		type To = DomainAddress;
	}

	impl pallet_mock_liquidity_pools::Config for Runtime {
		type DomainAddress = DomainAddress;
		type Message = Message;
	}

	impl pallet_mock_routers::Config for Runtime {}

	impl pallet_liquidity_pools_gateway::Config for Runtime {
		type AdminOrigin = EnsureRoot<AccountId>;
		type InboundQueue = MockLP;
		type LocalEVMOrigin = pallet_liquidity_pools_gateway::EnsureLocal;
		type MaxIncomingMessageSize = ConstU32<1024>;
		type Message = Message;
		type OriginRecovery = MockOriginRecovery;
		type OutboundMessageNonce = OutboundMessageNonce;
		type Router = RouterMock<Runtime>;
		type RuntimeEvent = RuntimeEvent;
		type RuntimeOrigin = RuntimeOrigin;
		type Sender = ConstU64<11>;
		type WeightInfo = ();
	}

	const RELAYER_PARA_ID: u32 = 1000;
	const RELAYER_EVM_ID: u64 = 1001;
	const RELAYER_ADDRESS: [u8; 20] = [1u8; 20];

	#[test]
	fn lp_instance_relayer_converts_correctly() {
		System::externalities().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Ok(RELAYER_EVM_ID)
			});

			let location = Location::new(
				1,
				[
					Parachain(RELAYER_PARA_ID),
					AccountKey20 {
						network: None,
						key: RELAYER_ADDRESS,
					},
				],
			);

			let origin = LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
				location,
				OriginKind::SovereignAccount,
			)
			.expect("Origin conversion failed unexpectedly.");

			assert_eq!(
				EnsureLocal::ensure_origin(origin).expect("Generate origin must be GatewayOrigin"),
				GatewayOrigin::AxelarRelay(expected_address)
			)
		})
	}

	#[test]
	fn lp_instance_relayer_fails_with_wrong_location() {
		System::externalities().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Ok(RELAYER_EVM_ID)
			});

			let location = Location::new(1, Parachain(RELAYER_PARA_ID));

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location.clone(),
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}

	#[test]
	fn lp_instance_relayer_fails_if_relayer_not_set() {
		System::externalities().execute_with(|| {
			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Ok(RELAYER_EVM_ID)
			});

			let location = Location::new(
				1,
				[
					Parachain(RELAYER_PARA_ID),
					AccountKey20 {
						network: None,
						key: RELAYER_ADDRESS,
					},
				],
			);

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location.clone(),
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}

	#[test]
	fn lp_instance_relayer_fails_if_para_to_evm_fails() {
		System::externalities().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Err(DispatchError::Other(""))
			});

			let location = Location::new(
				1,
				[
					Parachain(RELAYER_PARA_ID),
					AccountKey20 {
						network: None,
						key: RELAYER_ADDRESS,
					},
				],
			);

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location.clone(),
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}

	#[test]
	fn lp_instance_relayer_fails_if_wrong_para() {
		System::externalities().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, 1);
				Err(DispatchError::Other(""))
			});

			let location = Location::new(
				1,
				[
					Parachain(1),
					AccountKey20 {
						network: None,
						key: RELAYER_ADDRESS,
					},
				],
			);

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location.clone(),
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}

	#[test]
	fn lp_instance_relayer_fails_if_wrong_address() {
		System::externalities().execute_with(|| {
			let expected_address = DomainAddress::EVM(RELAYER_EVM_ID, RELAYER_ADDRESS);

			assert_ok!(Gateway::add_relayer(
				RuntimeOrigin::root(),
				expected_address.clone(),
			));

			MockParaAsEvmChain::mock_try_convert(|from| {
				assert_eq!(from, RELAYER_PARA_ID);
				Ok(RELAYER_EVM_ID)
			});

			let location = Location::new(
				1,
				[
					Parachain(RELAYER_PARA_ID),
					AccountKey20 {
						network: None,
						key: [0u8; 20],
					},
				],
			);

			assert_eq!(
				LpInstanceRelayer::<MockParaAsEvmChain, Runtime>::convert_origin(
					location.clone(),
					OriginKind::SovereignAccount,
				)
				.unwrap_err(),
				location
			);
		})
	}
}
