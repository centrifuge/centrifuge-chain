use crate::generic::{
	environment::Env,
	envs::{
		fudge_env::{FudgeEnv, FudgeSupport},
		runtime_env::RuntimeEnv,
	},
	runtime::Runtime,
	utils::genesis::{self, Genesis, MUSD_CURRENCY_ID},
};

fn what<T: Runtime + FudgeSupport>() {
	let env = RuntimeEnv::<T>::from_storage(
		Genesis::<T>::default()
			.add(genesis::assets(vec![MUSD_CURRENCY_ID]))
			.storage(),
	);

	env.state(|| {
		orml_asset_registry::Pallet::<T>::metadata(MUSD_CURRENCY_ID).unwrap();
	});
}

crate::test_for_runtimes!(all, what);
