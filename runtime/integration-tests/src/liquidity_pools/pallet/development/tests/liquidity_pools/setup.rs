// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.
//
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.
//
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::{currency_decimals, Balance, Moment, PoolId, TrancheId};
use cfg_traits::{investments::InvestmentAccountant, PoolMutate};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Rate,
	pools::TrancheMetadata,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
};
use cumulus_primitives_core::Junction::GlobalConsensus;
use development_runtime::{
	LiquidityPools, LiquidityPoolsGateway, OrmlAssetRegistry, OrmlTokens, PoolSystem,
	Runtime as DevelopmentRuntime, RuntimeOrigin, TreasuryPalletId,
};
use frame_support::{
	assert_ok,
	traits::{
		fungible::Mutate as _,
		fungibles::{Balanced, Mutate},
		Get, PalletInfo,
	},
};
use liquidity_pools_gateway_routers::{
	ethereum_xcm::EthereumXCMRouter, DomainRouter, XCMRouter, XcmDomain as GatewayXcmDomain,
	XcmTransactInfo,
};
use orml_asset_registry::{AssetMetadata, Metadata};
use pallet_liquidity_pools::Message;
use pallet_pool_system::tranches::{TrancheInput, TrancheType};
use runtime_common::{
	account_conversion::AccountConverter, xcm::general_key, xcm_fees::default_per_second,
};
use sp_core::H160;
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, ConstU32, Convert, EnsureAdd, One, Zero},
	BoundedVec, DispatchError, Perquintill, SaturatedConversion, WeakBoundedVec,
};
use xcm::{
	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId},
	prelude::{Parachain, X1, X2},
	VersionedMultiLocation,
};

use crate::{
	chain::centrifuge::development,
	liquidity_pools::pallet::development::{
		setup::{dollar, ALICE, BOB, PARA_ID_MOONBEAM},
		tests::register_ausd,
	},
	utils::{AUSD_CURRENCY_ID, GLMR_CURRENCY_ID, MOONBEAM_EVM_CHAIN_ID},
};

pub const DEFAULT_BALANCE_GLMR: Balance = 10_000_000_000_000_000_000;
pub const DOMAIN_MOONBEAM: Domain = Domain::EVM(MOONBEAM_EVM_CHAIN_ID);
pub const DEFAULT_EVM_ADDRESS_MOONBEAM: [u8; 20] = [99; 20];
pub const DEFAULT_DOMAIN_ADDRESS_MOONBEAM: DomainAddress =
	DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, DEFAULT_EVM_ADDRESS_MOONBEAM);
pub const DEFAULT_VALIDITY: Moment = 2555583502;
pub const DEFAULT_OTHER_DOMAIN_ADDRESS: DomainAddress =
	DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [0; 20]);
pub const DEFAULT_POOL_ID: u64 = 42;
pub const DEFAULT_MOONBEAM_LOCATION: MultiLocation = MultiLocation {
	parents: 1,
	interior: X1(Parachain(PARA_ID_MOONBEAM)),
};

pub type LiquidityPoolMessage = Message<Domain, PoolId, TrancheId, Balance, Rate>;

pub fn get_default_moonbeam_native_token_location() -> MultiLocation {
	MultiLocation {
		parents: 1,
		interior: X2(Parachain(PARA_ID_MOONBEAM), general_key(&[0, 1])),
	}
}

pub fn set_test_domain_router(
	evm_chain_id: u64,
	xcm_domain_location: VersionedMultiLocation,
	currency_id: CurrencyId,
	fee_location: VersionedMultiLocation,
) {
	let ethereum_xcm_router = EthereumXCMRouter::<DevelopmentRuntime> {
		router: XCMRouter {
			xcm_domain: GatewayXcmDomain {
				location: Box::new(xcm_domain_location),
				ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
				contract_address: H160::from(DEFAULT_EVM_ADDRESS_MOONBEAM),
				max_gas_limit: 700_000,
				transact_info: XcmTransactInfo {
					transact_extra_weight: 1.into(),
					max_weight: 8_000_000_000_000_000.into(),
					transact_extra_weight_signed: Some(3.into()),
				},
				fee_currency: currency_id,
				fee_per_second: default_per_second(18),
				fee_asset_location: Box::new(fee_location),
			},
			_marker: Default::default(),
		},
		_marker: Default::default(),
	};

	let domain_router = DomainRouter::EthereumXCM(ethereum_xcm_router);
	let domain = Domain::EVM(evm_chain_id);

	assert_ok!(LiquidityPoolsGateway::set_domain_router(
		RuntimeOrigin::root(),
		domain,
		domain_router,
	));
}

/// Initializes universally required storage for liquidityPools tests:
/// * Set the EthereumXCM router which in turn sets:
///     * transact info and domain router for Moonbeam `MultiLocation`,
///     * fee for GLMR (`GLIMMER_CURRENCY_ID`),
/// * Register GLMR and AUSD in `OrmlAssetRegistry`,
/// * Mint 10 GLMR (`DEFAULT_BALANCE_GLMR`) for Alice, Bob and the Treasury.
///
/// NOTE: AUSD is the default pool currency in `create_pool`.
/// Neither AUSD nor GLMR are registered as a liquidityPools-transferable
/// currency!
pub fn setup_pre_requirements() {
	/// Set the EthereumXCM router necessary for Moonbeam.
	set_test_domain_router(
		MOONBEAM_EVM_CHAIN_ID,
		DEFAULT_MOONBEAM_LOCATION.into(),
		GLMR_CURRENCY_ID,
		get_default_moonbeam_native_token_location().into(),
	);

	/// Register Moonbeam's native token
	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		asset_metadata(
			"Glimmer".into(),
			"GLMR".into(),
			18,
			false,
			Some(VersionedMultiLocation::V3(
				get_default_moonbeam_native_token_location()
			)),
			CrossChainTransferability::Xcm(Default::default()),
		),
		Some(GLMR_CURRENCY_ID)
	));

	// Give Alice, Bob and Treasury enough glimmer to pay for fees
	OrmlTokens::deposit(GLMR_CURRENCY_ID, &ALICE.into(), DEFAULT_BALANCE_GLMR);
	OrmlTokens::deposit(GLMR_CURRENCY_ID, &BOB.into(), DEFAULT_BALANCE_GLMR);
	// Treasury pays for `Executed*` messages
	OrmlTokens::deposit(
		GLMR_CURRENCY_ID,
		&TreasuryPalletId::get().into_account_truncating(),
		DEFAULT_BALANCE_GLMR,
	);

	// Register AUSD in the asset registry which is the default pool currency in
	// `create_pool`
	register_ausd();
}

/// Creates a new pool for the given id with
///  * BOB as admin and depositor
///  * Two tranches
///  * AUSD as pool currency with max reserve 10k.
pub fn create_ausd_pool(pool_id: u64) {
	create_currency_pool(pool_id, AUSD_CURRENCY_ID, dollar(currency_decimals::AUSD))
}

/// Creates a new pool for for the given id with the provided currency.
///  * BOB as admin and depositor
///  * Two tranches
///  * The given `currency` as pool currency with of `currency_decimals`.
pub fn create_currency_pool(pool_id: u64, currency_id: CurrencyId, currency_decimals: Balance) {
	assert_ok!(PoolSystem::create(
		BOB.into(),
		BOB.into(),
		pool_id,
		vec![
			TrancheInput {
				tranche_type: TrancheType::Residual,
				seniority: None,
				metadata:
					TrancheMetadata {
						// NOTE: For now, we have to set these metadata fields of the first tranche
						// to be convertible to the 32-byte size expected by the liquidity pools
						// AddTranche message.
						token_name: BoundedVec::<
							u8,
							cfg_types::consts::pools::MaxTrancheNameLengthBytes,
						>::try_from("A highly advanced tranche".as_bytes().to_vec())
						.expect(""),
						token_symbol: BoundedVec::<
							u8,
							cfg_types::consts::pools::MaxTrancheSymbolLengthBytes,
						>::try_from("TrNcH".as_bytes().to_vec())
						.expect(""),
					}
			},
			TrancheInput {
				tranche_type: TrancheType::NonResidual {
					interest_rate_per_sec: One::one(),
					min_risk_buffer: Perquintill::from_percent(10),
				},
				seniority: None,
				metadata: TrancheMetadata {
					token_name: BoundedVec::default(),
					token_symbol: BoundedVec::default(),
				}
			}
		],
		currency_id,
		currency_decimals,
	));
}

/// Returns a `VersionedMultiLocation` that can be converted into
/// `LiquidityPoolsWrappedToken` which is required for cross chain asset
/// registration and transfer.
pub fn liquidity_pools_transferable_multilocation(
	chain_id: u64,
	address: [u8; 20],
) -> VersionedMultiLocation {
	VersionedMultiLocation::V3(MultiLocation {
		parents: 0,
		interior: X3(
			PalletInstance(
				<DevelopmentRuntime as frame_system::Config>::PalletInfo::index::<LiquidityPools>()
					.expect("LiquidityPools should have pallet index")
					.saturated_into(),
			),
			GlobalConsensus(NetworkId::Ethereum { chain_id }),
			AccountKey20 {
				network: None,
				key: address,
			},
		),
	})
}

/// Enables `LiquidityPoolsTransferable` in the custom asset metadata for
/// the given currency_id.
///
/// NOTE: Sets the location to the `MOONBEAM_EVM_CHAIN_ID` with dummy
/// address as the location is required for LiquidityPoolsWrappedToken
/// conversions.
pub fn enable_liquidity_pool_transferability(currency_id: CurrencyId) {
	let metadata =
		Metadata::<DevelopmentRuntime>::get(currency_id).expect("Currency should be registered");
	let location = Some(Some(liquidity_pools_transferable_multilocation(
		MOONBEAM_EVM_CHAIN_ID,
		// Value of evm_address is irrelevant here
		[1u8; 20],
	)));

	assert_ok!(OrmlAssetRegistry::update_asset(
		RuntimeOrigin::root(),
		currency_id,
		None,
		None,
		None,
		None,
		location,
		Some(CustomMetadata {
			// Changed: Allow liquidity_pools transferability
			transferability: CrossChainTransferability::LiquidityPools,
			..metadata.additional
		})
	));
}

/// Returns metadata for the given data with existential deposit of
/// 1_000_000.
pub fn asset_metadata(
	name: Vec<u8>,
	symbol: Vec<u8>,
	decimals: u32,
	is_pool_currency: bool,
	location: Option<VersionedMultiLocation>,
	transferability: CrossChainTransferability,
) -> AssetMetadata<Balance, CustomMetadata> {
	AssetMetadata {
		name,
		symbol,
		decimals,
		location,
		existential_deposit: 1_000_000,
		additional: CustomMetadata {
			transferability,
			mintable: false,
			permissioned: false,
			pool_currency: is_pool_currency,
		},
	}
}

pub(crate) mod investments {
	use cfg_primitives::AccountId;
	use cfg_traits::investments::TrancheCurrency as TrancheCurrencyT;
	use cfg_types::investments::InvestmentAccount;
	use development_runtime::PoolSystem;
	use pallet_pool_system::tranches::TrancheLoc;

	use super::*;

	/// Returns the default investment account derived from the
	/// `DEFAULT_POOL_ID` and its default tranche.
	pub fn default_investment_account() -> AccountId {
		InvestmentAccount {
			investment_id: default_investment_id(),
		}
		.into_account_truncating()
	}

	/// Returns the investment_id of the given pool and tranche ids.
	pub fn investment_id(
		pool_id: u64,
		tranche_id: TrancheId,
	) -> cfg_types::tokens::TrancheCurrency {
		<DevelopmentRuntime as pallet_liquidity_pools::Config>::TrancheCurrency::generate(
			pool_id, tranche_id,
		)
	}

	pub fn default_investment_id() -> cfg_types::tokens::TrancheCurrency {
		<DevelopmentRuntime as pallet_liquidity_pools::Config>::TrancheCurrency::generate(
			DEFAULT_POOL_ID,
			default_tranche_id(DEFAULT_POOL_ID),
		)
	}

	/// Returns the tranche id at index 0 for the given pool id.
	pub fn default_tranche_id(pool_id: u64) -> TrancheId {
		let pool_details = PoolSystem::pool(pool_id).expect("Pool should exist");
		pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists")
	}

	/// Returns the derived general currency index.
	///
	/// Throws if the provided currency_id is not
	/// `CurrencyId::ForeignAsset(id)`.
	pub fn general_currency_index(currency_id: CurrencyId) -> u128 {
		pallet_liquidity_pools::Pallet::<DevelopmentRuntime>::try_get_general_index(currency_id)
			.expect("ForeignAsset should convert into u128")
	}
}
