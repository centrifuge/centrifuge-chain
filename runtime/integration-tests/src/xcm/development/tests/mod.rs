use cfg_primitives::{parachains, Balance};
use cfg_types::tokens::{CrossChainTransferability, CustomMetadata};
use development_runtime::{OrmlAssetRegistry, RuntimeOrigin};
use frame_support::assert_ok;
use orml_traits::asset_registry::AssetMetadata;
use runtime_common::xcm::general_key;
use xcm::{
	latest::MultiLocation,
	prelude::{Parachain, X2},
	VersionedMultiLocation,
};

use crate::utils::AUSD_CURRENCY_ID;

mod connectors;

/// Register AUSD in the asset registry.
/// It should be executed within an externalities environment.
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
