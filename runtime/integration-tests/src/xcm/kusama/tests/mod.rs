use centrifuge_runtime::{OrmlAssetRegistry, RuntimeOrigin};
use cfg_primitives::{parachains, Balance};
use cfg_types::{
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use frame_support::assert_ok;
use orml_traits::asset_registry::AssetMetadata;
use runtime_common::{xcm::general_key, xcm_fees::ksm_per_second};
use xcm::{
	latest::MultiLocation,
	prelude::{Here, Parachain, X2},
	VersionedMultiLocation,
};

use crate::{utils::AUSD_CURRENCY_ID, xcm::kusama::setup::KSM_ASSET_ID};

mod asset_registry;
mod currency_id_convert;
mod transfers;

/// Register AIR in the asset registry.
/// It should be executed within an externalities environment.
fn register_air() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 18,
		name: "Altair".into(),
		symbol: "AIR".into(),
		existential_deposit: 1_000_000_000_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::new(
			1,
			X2(
				Parachain(parachains::kusama::altair::ID),
				general_key(parachains::kusama::altair::AIR_KEY),
			),
		))),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::Xcm(Default::default()),
			..CustomMetadata::default()
		},
	};

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(CurrencyId::Native)
	));
}

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
			..CustomMetadata::default()
		},
	};

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(AUSD_CURRENCY_ID)
	));
}

/// Register KSM in the asset registry.
/// It should be executed within an externalities environment.
fn register_ksm() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 12,
		name: "Kusama".into(),
		symbol: "KSM".into(),
		existential_deposit: 1_000_000_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::new(1, Here))),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::Xcm(Default::default()),
			..CustomMetadata::default()
		},
	};

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(KSM_ASSET_ID)
	));
}
