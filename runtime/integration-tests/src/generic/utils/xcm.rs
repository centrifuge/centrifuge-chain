use cfg_primitives::AccountId;
use cfg_types::tokens::{AssetMetadata, CrossChainTransferability, CustomMetadata};
use frame_support::{assert_ok, dispatch::RawOrigin};
use polkadot_parachain_primitives::primitives::Id;
use staging_xcm::{
	prelude::XCM_VERSION,
	v4::{Junction::*, Location},
	VersionedLocation,
};

use crate::generic::{
	config::Runtime,
	env::{Blocks, Env},
	envs::fudge_env::{handle::FudgeHandle, FudgeEnv, FudgeRelayRuntime, FudgeSupport},
	utils::currency::default_metadata,
};

pub fn enable_relay_to_para_communication<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
	env.relay_state_mut(|| {
		assert_ok!(
			pallet_xcm::Pallet::<FudgeRelayRuntime<T>>::force_xcm_version(
				RawOrigin::Root.into(),
				Box::new(Location::new(0, Parachain(T::FudgeHandle::PARA_ID))),
				XCM_VERSION,
			)
		);
	});
}

pub fn enable_para_to_relay_communication<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
	env.parachain_state_mut(|| {
		assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
			RawOrigin::Root.into(),
			Box::new(Location::parent()),
			XCM_VERSION,
		));
	});
}

pub fn enable_para_to_sibling_communication<T: Runtime + FudgeSupport>(env: &mut FudgeEnv<T>) {
	env.parachain_state_mut(|| {
		assert_ok!(pallet_xcm::Pallet::<T>::force_xcm_version(
			RawOrigin::Root.into(),
			Box::new(Location::new(1, Parachain(T::FudgeHandle::SIBLING_ID))),
			XCM_VERSION,
		));
	});

	env.relay_state_mut(|| {
		// Enable para -> sibling comunication though relay
		assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
			FudgeRelayRuntime<T>,
		>::force_open_hrmp_channel(
			RawOrigin::Root.into(),
			Id::from(T::FudgeHandle::PARA_ID),
			Id::from(T::FudgeHandle::SIBLING_ID),
			10,
			1024,
		));

		assert_ok!(polkadot_runtime_parachains::hrmp::Pallet::<
			FudgeRelayRuntime<T>,
		>::force_process_hrmp_open(RawOrigin::Root.into(), 1));
	});

	env.pass(Blocks::ByNumber(1));
}

pub fn account_location(
	parents: u8,
	para_id: Option<u32>,
	account_id: AccountId,
) -> Box<VersionedLocation> {
	let account = AccountId32 {
		network: None,
		id: account_id.into(),
	};

	Box::new(VersionedLocation::V4(match para_id {
		Some(para_id) => Location::new(parents, [Parachain(para_id), account]),
		None => Location::new(parents, account),
	}))
}

pub fn transferable_metadata(origin_para_id: Option<u32>) -> AssetMetadata {
	let location = match origin_para_id {
		Some(para_id) => Location::new(1, Parachain(para_id)),
		None => Location::parent(),
	};

	AssetMetadata {
		location: Some(VersionedLocation::V4(location)),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::xcm_with_fees(0),
			..Default::default()
		},
		..default_metadata()
	}
}
