use cfg_types::tokens::{default_metadata, CurrencyId};
use orml_traits::asset_registry::AssetMetadata;
use runtime_common::xcm::CurrencyIdConvert;
use sp_runtime::traits::Convert;
use staging_xcm::{
	v4::{Junction::*, Junctions::Here, Location},
	VersionedLocation,
};

use crate::generic::{
	config::Runtime,
	env::Env,
	envs::runtime_env::RuntimeEnv,
	utils::{
		currency::{CurrencyInfo, CustomCurrency},
		genesis::{self, Genesis},
		xcm::transferable_custom,
	},
};

const PARA_ID: u32 = 1000;

#[test_runtimes(all)]
fn convert_transferable_asset<T: Runtime>() {
	// The way the native currency is represented relative to its runtime
	let location_inner = Location::new(0, Here);

	// The canonical way the native currency is represented out in the wild
	let location_canonical = Location::new(1, Parachain(PARA_ID));

	let curr = CustomCurrency(
		CurrencyId::ForeignAsset(1),
		AssetMetadata {
			decimals: 18,
			location: Some(VersionedLocation::V4(location_canonical.clone())),
			additional: transferable_custom(),
			..default_metadata()
		},
	);

	let env = RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::parachain_id::<T>(PARA_ID))
			.add(genesis::assets::<T>([(curr.id(), curr.metadata())]))
			.storage(),
	);

	env.parachain_state(|| {
		assert_eq!(
			CurrencyIdConvert::<T>::convert(location_inner),
			Some(curr.id()),
		);

		assert_eq!(
			CurrencyIdConvert::<T>::convert(curr.id()),
			Some(location_canonical)
		)
	});
}

#[test_runtimes(all)]
fn cannot_convert_nontransferable_asset<T: Runtime>() {
	let curr = CustomCurrency(
		CurrencyId::ForeignAsset(1),
		AssetMetadata {
			decimals: 18,
			location: Some(VersionedLocation::V4(Location::new(1, Parachain(PARA_ID)))),
			additional: Default::default(), // <- Not configured for transfers
			..default_metadata()
		},
	);

	let env = RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::parachain_id::<T>(PARA_ID))
			.add(genesis::assets::<T>([(curr.id(), curr.metadata())]))
			.storage(),
	);

	env.parachain_state(|| {
		assert_eq!(
			CurrencyIdConvert::<T>::convert(Location::new(0, Here)),
			Some(curr.id()),
		);

		assert_eq!(CurrencyIdConvert::<T>::convert(curr.id()), None);
	});
}

#[test_runtimes(all)]
fn convert_unknown_location<T: Runtime>() {
	let env = RuntimeEnv::<T>::default();

	env.parachain_state(|| {
		assert_eq!(
			CurrencyIdConvert::<T>::convert(Location::new(0, Here)),
			None,
		);
	});
}
