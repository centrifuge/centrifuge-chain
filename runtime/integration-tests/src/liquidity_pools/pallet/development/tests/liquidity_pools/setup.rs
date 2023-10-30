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

use cfg_primitives::{currency_decimals, Balance, PoolId, TrancheId};
use cfg_traits::{investments::InvestmentAccountant, PoolMutate, Seconds};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::{Quantity, Rate},
	pools::TrancheMetadata,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
};
use cumulus_primitives_core::Junction::GlobalConsensus;
use frame_support::{
	assert_ok,
	traits::{
		fungible::Mutate as _,
		fungibles::{Balanced, Mutate},
		Get, PalletInfo,
	},
	weights::Weight,
};
use fudge::primitives::Chain;
use liquidity_pools_gateway_routers::{
	ethereum_xcm::EthereumXCMRouter, DomainRouter, XCMRouter, XcmDomain as GatewayXcmDomain,
	XcmTransactInfo, DEFAULT_PROOF_SIZE,
};
use orml_asset_registry::{AssetMetadata, Metadata};
use orml_traits::MultiCurrency;
use pallet_liquidity_pools::Message;
use pallet_pool_system::tranches::{TrancheInput, TrancheType};
use polkadot_parachain::primitives::Id;
use runtime_common::{
	account_conversion::AccountConverter, xcm::general_key, xcm_fees::default_per_second,
};
use sp_core::H160;
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, ConstU32, Convert, EnsureAdd, One, Zero},
	BoundedVec, DispatchError, Perquintill, SaturatedConversion, WeakBoundedVec,
};
use xcm::{
	prelude::{Parachain, X1, X2, X3, XCM_VERSION},
	v3::{Junction, Junction::*, Junctions, MultiLocation, NetworkId},
	VersionedMultiLocation,
};

use crate::{
	chain::{
		centrifuge::{
			LiquidityPools, LiquidityPoolsGateway, OrmlAssetRegistry, OrmlTokens, PolkadotXcm,
			PoolSystem, Runtime as DevelopmentRuntime, RuntimeOrigin, Tokens, TreasuryPalletId,
			PARA_ID,
		},
		relay::{Hrmp as RelayHrmp, RuntimeOrigin as RelayRuntimeOrigin},
	},
	liquidity_pools::pallet::development::{setup::dollar, tests::register_ausd},
	utils::{
		accounts::Keyring,
		env::{TestEnv, PARA_ID_SIBLING},
		AUSD_CURRENCY_ID, GLMR_CURRENCY_ID, MOONBEAM_EVM_CHAIN_ID,
	},
};

// 10 GLMR (18 decimals)
pub const DEFAULT_BALANCE_GLMR: Balance = 10_000_000_000_000_000_000;
pub const DOMAIN_MOONBEAM: Domain = Domain::EVM(MOONBEAM_EVM_CHAIN_ID);
pub const DEFAULT_EVM_ADDRESS_MOONBEAM: [u8; 20] = [99; 20];
pub const DEFAULT_DOMAIN_ADDRESS_MOONBEAM: DomainAddress =
	DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, DEFAULT_EVM_ADDRESS_MOONBEAM);
pub const DEFAULT_VALIDITY: Seconds = 2555583502;
pub const DEFAULT_OTHER_DOMAIN_ADDRESS: DomainAddress =
	DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [0; 20]);
pub const DEFAULT_POOL_ID: u64 = 42;
pub const DEFAULT_SIBLING_LOCATION: MultiLocation = MultiLocation {
	parents: 1,
	interior: X1(Parachain(PARA_ID_SIBLING)),
};

pub type LiquidityPoolMessage = Message<Domain, PoolId, TrancheId, Balance, Quantity>;

pub fn get_default_moonbeam_native_token_location() -> MultiLocation {
	MultiLocation {
		parents: 1,
		interior: X2(Parachain(PARA_ID_SIBLING), general_key(&[0, 1])),
	}
}

pub fn set_test_domain_router(
	evm_chain_id: u64,
	xcm_domain_location: VersionedMultiLocation,
	currency_id: CurrencyId,
) {
	let ethereum_xcm_router = EthereumXCMRouter::<DevelopmentRuntime> {
		router: XCMRouter {
			xcm_domain: GatewayXcmDomain {
				location: Box::new(xcm_domain_location),
				ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
				contract_address: H160::from(DEFAULT_EVM_ADDRESS_MOONBEAM),
				max_gas_limit: 500_000,
				transact_required_weight_at_most: Weight::from_parts(
					12530000000,
					DEFAULT_PROOF_SIZE.saturating_div(2),
				),
				overall_weight: Weight::from_parts(15530000000, DEFAULT_PROOF_SIZE),
				fee_currency: currency_id,
				// 0.2 token
				fee_amount: 200000000000000000,
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

pub fn setup_test_env(env: &mut TestEnv) {
	env.with_mut_state(Chain::Para(PARA_ID), || {
		setup_pre_requirements();
	})
	.unwrap();

	env.with_mut_state(Chain::Relay, || {
		setup_hrmp_channel();
	})
	.unwrap();

	env.evolve().unwrap();
}

/// Initializes universally required storage for liquidityPools tests:
/// * Set the EthereumXCM router which in turn sets:
///     * transact info and domain router for Moonbeam `MultiLocation`,
///     * fee for GLMR (`GLMR_CURRENCY_ID`),
/// * Register GLMR and AUSD in `OrmlAssetRegistry`,
/// * Mint 10 GLMR (`DEFAULT_BALANCE_GLMR`) for the LP Gateway Sender.
/// * Set the XCM version for the sibling parachain.
///
/// NOTE: AUSD is the default pool currency in `create_pool`.
/// Neither AUSD nor GLMR are registered as a liquidityPools-transferable
/// currency!
pub fn setup_pre_requirements() {
	/// Set the EthereumXCM router necessary for Moonbeam.
	set_test_domain_router(
		MOONBEAM_EVM_CHAIN_ID,
		DEFAULT_SIBLING_LOCATION.into(),
		GLMR_CURRENCY_ID,
	);

	/// Register Moonbeam's native token
	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		asset_metadata(
			"Glimmer".into(),
			"GLMR".into(),
			18,
			false,
			GLMR_ED,
			Some(VersionedMultiLocation::V3(
				get_default_moonbeam_native_token_location()
			)),
			CrossChainTransferability::Xcm(Default::default()),
		),
		Some(GLMR_CURRENCY_ID)
	));

	// Fund the gateway sender account with enough glimmer to pay for fees
	assert_ok!(Tokens::set_balance(
		RuntimeOrigin::root(),
		<DevelopmentRuntime as pallet_liquidity_pools_gateway::Config>::Sender::get().into(),
		GLMR_CURRENCY_ID,
		DEFAULT_BALANCE_GLMR,
		0,
	));

	// Register AUSD in the asset registry which is the default pool currency in
	// `create_pool`
	register_ausd();

	// Set the XCM version used when sending XCM messages to sibling.
	assert_ok!(PolkadotXcm::force_xcm_version(
		RuntimeOrigin::root(),
		Box::new(MultiLocation::new(
			1,
			Junctions::X1(Junction::Parachain(PARA_ID_SIBLING)),
		)),
		XCM_VERSION,
	));
}

/// Opens the required HRMP channel between parachain and sibling.
///
/// NOTE - this is should be done on the relay chain.
pub fn setup_hrmp_channel() {
	assert_ok!(RelayHrmp::force_open_hrmp_channel(
		RelayRuntimeOrigin::root(),
		Id::from(PARA_ID),
		Id::from(PARA_ID_SIBLING),
		10,
		1024,
	));

	assert_ok!(RelayHrmp::force_process_hrmp_open(
		RelayRuntimeOrigin::root(),
		0,
	));
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
		Keyring::Bob.into(),
		Keyring::Bob.into(),
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
	existential_deposit: Balance,
	location: Option<VersionedMultiLocation>,
	transferability: CrossChainTransferability,
) -> AssetMetadata<Balance, CustomMetadata> {
	AssetMetadata {
		name,
		symbol,
		decimals,
		location,
		existential_deposit,
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
	use development_runtime::{OrderBook, PoolSystem};
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
