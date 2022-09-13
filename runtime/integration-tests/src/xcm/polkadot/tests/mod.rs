use super::setup::DOT_ASSET_ID;
use centrifuge_runtime::{Origin, OrmlAssetRegistry};
use common_types::{CustomMetadata, XcmMetadata};
use frame_support::assert_ok;
use orml_traits::asset_registry::AssetMetadata;
use runtime_common::{xcm_fees::ksm_per_second, Balance};
use xcm::latest::MultiLocation;
use xcm::VersionedMultiLocation;

mod asset_registry;
mod currency_id_convert;
mod restricted_calls;
mod transfers;

/// Register DOT in the asset registry.
/// It should be executed within an externalities environment.
fn register_dot() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 12,
		name: "Polkadot".into(),
		symbol: "DOT".into(),
		existential_deposit: 100_000_000,
		location: Some(VersionedMultiLocation::V1(MultiLocation::parent())),
		additional: CustomMetadata {
			xcm: XcmMetadata {
				// We specify a custom fee_per_second and verify below that this value is
				// used when XCM transfer fees are charged for this token.
				fee_per_second: Some(ksm_per_second()),
			},
			..CustomMetadata::default()
		},
	};
	assert_ok!(OrmlAssetRegistry::register_asset(
		Origin::root(),
		meta,
		Some(DOT_ASSET_ID)
	));
}
