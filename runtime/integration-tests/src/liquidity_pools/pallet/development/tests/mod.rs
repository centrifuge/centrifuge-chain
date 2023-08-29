use cfg_primitives::{parachains, Balance};
use cfg_types::tokens::{CrossChainTransferability, CustomMetadata};
use development_runtime::{OrmlAssetRegistry, RuntimeOrigin};
use frame_support::assert_ok;
use orml_traits::asset_registry::AssetMetadata;
use runtime_common::xcm::general_key;
use xcm::{
	latest::MultiLocation,
	prelude::{GeneralIndex, PalletInstance, Parachain, X2, X3},
	VersionedMultiLocation,
};

use crate::utils::{AUSD_CURRENCY_ID, USDT_CURRENCY_ID};

mod liquidity_pools;
mod routers;

/// Register AUSD in the asset registry.
///
/// NOTE: Assumes to be executed within an externalities environment.
fn register_ausd() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 12,
		name: "Acala Dollar".into(),
		symbol: "AUSD".into(),
		existential_deposit: 1_000_000_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::new(
			1,
			X2(
				Parachain(parachains::kusama::karura::ID),
				general_key(parachains::kusama::karura::AUSD_KEY),
			),
		))),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::Xcm(Default::default()),
			pool_currency: true,
			..CustomMetadata::default()
		},
	};

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(AUSD_CURRENCY_ID)
	));
}

/// Register USDT in the asset registry and enable LiquidityPools cross chain
/// transferability.
///
/// NOTE: Assumes to be executed within an externalities environment.
fn register_usdt() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 6,
		name: "Tether USDT".into(),
		symbol: "USDT".into(),
		existential_deposit: 10_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::new(
			1,
			X3(Parachain(1000), PalletInstance(50), GeneralIndex(1984)),
		))),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::LiquidityPools,
			pool_currency: true,
			..CustomMetadata::default()
		},
	};

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(USDT_CURRENCY_ID)
	));
}
