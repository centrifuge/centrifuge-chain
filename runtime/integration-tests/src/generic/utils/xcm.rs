use frame_support::{assert_ok, traits::OriginTrait};
use polkadot_parachain_primitives::primitives::Id;
use staging_xcm::{
	prelude::XCM_VERSION,
	v4::{Junction, Location},
};

use crate::generic::{
	config::Runtime,
	env::{Blocks, Env},
	envs::fudge_env::{handle::FudgeHandle, FudgeEnv, FudgeSupport},
};

type FudgeRelayRuntime<T> = <<T as FudgeSupport>::FudgeHandle as FudgeHandle<T>>::RelayRuntime;

pub fn setup_xcm<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
	env.parachain_state_mut(|| {
		// Set the XCM version used when sending XCM messages to sibling.
		assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			Box::new(Location::new(
				1,
				Junction::Parachain(T::FudgeHandle::SIBLING_ID),
			)),
			XCM_VERSION,
		));
	});

	env.sibling_state_mut(|| {
		// Set the XCM version used when sending XCM messages to parachain.
		assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			Box::new(Location::new(
				1,
				Junction::Parachain(T::FudgeHandle::PARA_ID),
			)),
			XCM_VERSION,
		));
	});

	env.relay_state_mut(|| {
		assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
			FudgeRelayRuntime<T>,
		>::force_open_hrmp_channel(
			<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
			Id::from(T::FudgeHandle::PARA_ID),
			Id::from(T::FudgeHandle::SIBLING_ID),
			10,
			1024,
		));

		assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
			FudgeRelayRuntime<T>,
		>::force_open_hrmp_channel(
			<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
			Id::from(T::FudgeHandle::SIBLING_ID),
			Id::from(T::FudgeHandle::PARA_ID),
			10,
			1024,
		));

		assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
			FudgeRelayRuntime<T>,
		>::force_process_hrmp_open(
			<FudgeRelayRuntime<T> as frame_system::Config>::RuntimeOrigin::root(),
			0,
		));
	});

	env.pass(Blocks::ByNumber(1));
}
