use cfg_types::tokens::CurrencyId;
use frame_support::{assert_noop, assert_ok, dispatch::RawOrigin};
use sp_runtime::{DispatchError, DispatchError::BadOrigin};

use crate::{
	generic::{
		config::Runtime, env::Env, envs::runtime_env::RuntimeEnv, utils::currency::default_metadata,
	},
	utils::orml_asset_registry,
};

#[test_runtimes(all)]
fn authority_configured<T: Runtime>() {
	let mut env = RuntimeEnv::<T>::default();

	env.parachain_state_mut(|| {
		assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
			RawOrigin::Root.into(),
			default_metadata(),
			Some(CurrencyId::Native)
		));

		assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
			RawOrigin::Root.into(),
			default_metadata(),
			Some(CurrencyId::ForeignAsset(42))
		));

		assert_noop!(
			orml_asset_registry::Pallet::<T>::register_asset(
				RawOrigin::Root.into(),
				default_metadata(),
				Some(CurrencyId::Tranche(42, [1; 16]))
			),
			BadOrigin
		);
	});
}

#[test_runtimes(all)]
fn processor_configured<T: Runtime>() {
	let mut env = RuntimeEnv::<T>::default();

	env.parachain_state_mut(|| {
		assert_noop!(
			orml_asset_registry::Pallet::<T>::register_asset(
				RawOrigin::Root.into(),
				default_metadata(),
				None
			),
			DispatchError::Other("asset-registry: AssetId is required")
		);
	});
}
