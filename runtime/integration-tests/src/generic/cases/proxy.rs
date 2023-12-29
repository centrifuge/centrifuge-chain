use cfg_primitives::Balance;
use cfg_types::tokens::{CrossChainTransferability, CustomMetadata};
use frame_support::{
	assert_ok,
	traits::{Get, OriginTrait},
};
use frame_system::RawOrigin;
use orml_traits::asset_registry::AssetMetadata;
use sp_runtime::{traits::StaticLookup, DispatchResult};
use xcm::{
	prelude::Parachain,
	v3::{Junction, Junctions::*, MultiLocation, WeightLimit},
	VersionedMultiLocation,
};

use crate::{
	generic::{
		cases::liquidity_pools::utils::setup_xcm,
		config::Runtime,
		env::Env,
		envs::{
			fudge_env::{handle::FudgeHandle, FudgeEnv, FudgeSupport},
			runtime_env::RuntimeEnv,
		},
		utils::{
			self,
			currency::{cfg, usd6, CurrencyInfo, Usd6},
			genesis::{self, Genesis},
		},
	},
	utils::accounts::Keyring,
};

const FROM: Keyring = Keyring::Charlie;
const PROXY: Keyring = Keyring::Alice;
const TO: Keyring = Keyring::Bob;

const FOR_FEES: Balance = cfg(1);
const TRANSFER_AMOUNT: Balance = usd6(100);

fn configure_proxy_and_transfer<T: Runtime>(proxy_type: T::ProxyType) -> DispatchResult {
	let env = RuntimeEnv::<T>::from_parachain_storage(
		Genesis::<T>::default()
			.add(genesis::balances(T::ExistentialDeposit::get() + FOR_FEES))
			.add(genesis::tokens(vec![(Usd6::ID, Usd6::ED)]))
			.storage(),
	);

	let call = pallet_restricted_tokens::Call::transfer {
		currency_id: Usd6::ID,
		amount: TRANSFER_AMOUNT,
		dest: T::Lookup::unlookup(TO.id()),
	}
	.into();

	configure_proxy_and_call::<T>(env, proxy_type, call)
}

fn configure_proxy_and_x_transfer<T: Runtime + FudgeSupport>(
	proxy_type: T::ProxyType,
) -> DispatchResult {
	let mut env = FudgeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(
				T::ExistentialDeposit::get() + FOR_FEES,
			))
			.add(genesis::tokens(vec![(Usd6::ID, Usd6::ED)]))
			.storage(),
	);

	setup_xcm(&mut env);

	env.parachain_state_mut(|| {
		register_currency::<T, Usd6>();
	});

	let call = pallet_restricted_xtokens::Call::transfer {
		currency_id: Usd6::ID,
		amount: TRANSFER_AMOUNT,
		dest: Box::new(
			MultiLocation::new(
				1,
				X2(
					Parachain(T::FudgeHandle::SIBLING_ID),
					Junction::AccountId32 {
						id: TO.into(),
						network: None,
					},
				),
			)
			.into(),
		),
		dest_weight_limit: WeightLimit::Unlimited,
	}
	.into();

	configure_proxy_and_call::<T>(env, proxy_type, call)
}

pub fn register_currency<T: Runtime + FudgeSupport, C: CurrencyInfo>() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: C::DECIMALS,
		name: C::NAME.into(),
		symbol: C::SYMBOL.into(),
		existential_deposit: C::ED,
		location: Some(VersionedMultiLocation::V3(MultiLocation::new(
			1,
			X1(Parachain(T::FudgeHandle::SIBLING_ID)),
		))),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::Xcm(Default::default()),
			..CustomMetadata::default()
		},
	};

	assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
		<T as frame_system::Config>::RuntimeOrigin::root(),
		meta,
		Some(C::ID)
	));
}

fn configure_proxy_and_call<T: Runtime>(
	mut env: impl Env<T>,
	proxy_type: T::ProxyType,
	call: T::RuntimeCallExt,
) -> DispatchResult {
	env.parachain_state_mut(|| {
		utils::give_tokens::<T>(FROM.id(), Usd6::ID, TRANSFER_AMOUNT);

		// Register PROXY as proxy of FROM
		pallet_proxy::Pallet::<T>::add_proxy(
			RawOrigin::Signed(FROM.id()).into(),
			T::Lookup::unlookup(PROXY.id()),
			proxy_type,
			0,
		)
		.unwrap();

		// Acts as FROM using PROXY
		pallet_proxy::Pallet::<T>::proxy(
			RawOrigin::Signed(PROXY.id()).into(),
			T::Lookup::unlookup(FROM.id()),
			None,
			Box::new(call),
		)
		.unwrap();
	});

	env.find_event(|e| match e {
		pallet_proxy::Event::<T>::ProxyExecuted { result } => Some(result),
		_ => None,
	})
	.unwrap()
}

fn development_transfer_with_proxy_transfer<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert!(configure_proxy_and_transfer::<T>(development_runtime::ProxyType::Transfer).is_ok());
}

fn development_transfer_with_proxy_borrow<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert!(configure_proxy_and_transfer::<T>(development_runtime::ProxyType::Borrow).is_err());
}

fn development_transfer_with_proxy_invest<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert!(configure_proxy_and_transfer::<T>(development_runtime::ProxyType::Invest).is_err());
}

fn development_x_transfer_with_proxy_transfer<T: Runtime + FudgeSupport>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert!(configure_proxy_and_x_transfer::<T>(development_runtime::ProxyType::Transfer).is_ok());
}

fn development_x_transfer_with_proxy_borrow<T: Runtime + FudgeSupport>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert!(configure_proxy_and_x_transfer::<T>(development_runtime::ProxyType::Borrow).is_err());
}

fn development_x_transfer_with_proxy_invest<T: Runtime + FudgeSupport>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert!(configure_proxy_and_x_transfer::<T>(development_runtime::ProxyType::Invest).is_err());
}

fn altair_transfer_with_proxy_transfer<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert!(configure_proxy_and_transfer::<T>(altair_runtime::ProxyType::Transfer).is_ok());
}

fn altair_transfer_with_proxy_borrow<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert!(configure_proxy_and_transfer::<T>(altair_runtime::ProxyType::Borrow).is_err());
}

fn altair_transfer_with_proxy_invest<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert!(configure_proxy_and_transfer::<T>(altair_runtime::ProxyType::Invest).is_err());
}

fn altair_x_transfer_with_proxy_transfer<T: Runtime + FudgeSupport>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert!(configure_proxy_and_x_transfer::<T>(altair_runtime::ProxyType::Transfer).is_ok());
}

fn altair_x_transfer_with_proxy_borrow<T: Runtime + FudgeSupport>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert!(configure_proxy_and_x_transfer::<T>(altair_runtime::ProxyType::Borrow).is_err());
}

fn altair_x_transfer_with_proxy_invest<T: Runtime + FudgeSupport>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert!(configure_proxy_and_x_transfer::<T>(altair_runtime::ProxyType::Invest).is_err());
}

fn centrifuge_transfer_with_proxy_transfer<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert!(configure_proxy_and_transfer::<T>(centrifuge_runtime::ProxyType::Transfer).is_ok());
}

fn centrifuge_transfer_with_proxy_borrow<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert!(configure_proxy_and_transfer::<T>(centrifuge_runtime::ProxyType::Borrow).is_err());
}

fn centrifuge_transfer_with_proxy_invest<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert!(configure_proxy_and_transfer::<T>(centrifuge_runtime::ProxyType::Invest).is_err());
}

fn centrifuge_x_transfer_with_proxy_transfer<T: Runtime + FudgeSupport>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert!(configure_proxy_and_x_transfer::<T>(centrifuge_runtime::ProxyType::Transfer).is_ok());
}

fn centrifuge_x_transfer_with_proxy_borrow<T: Runtime + FudgeSupport>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert!(configure_proxy_and_x_transfer::<T>(centrifuge_runtime::ProxyType::Borrow).is_err());
}

fn centrifuge_x_transfer_with_proxy_invest<T: Runtime + FudgeSupport>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert!(configure_proxy_and_x_transfer::<T>(centrifuge_runtime::ProxyType::Invest).is_err());
}

crate::test_for_runtimes!([development], development_transfer_with_proxy_transfer);
crate::test_for_runtimes!([development], development_transfer_with_proxy_borrow);
crate::test_for_runtimes!([development], development_transfer_with_proxy_invest);
crate::test_for_runtimes!([development], development_x_transfer_with_proxy_transfer);
crate::test_for_runtimes!([development], development_x_transfer_with_proxy_borrow);
crate::test_for_runtimes!([development], development_x_transfer_with_proxy_invest);

crate::test_for_runtimes!([altair], altair_transfer_with_proxy_transfer);
crate::test_for_runtimes!([altair], altair_transfer_with_proxy_borrow);
crate::test_for_runtimes!([altair], altair_transfer_with_proxy_invest);
crate::test_for_runtimes!([altair], altair_x_transfer_with_proxy_transfer);
crate::test_for_runtimes!([altair], altair_x_transfer_with_proxy_borrow);
crate::test_for_runtimes!([altair], altair_x_transfer_with_proxy_invest);

crate::test_for_runtimes!([centrifuge], centrifuge_transfer_with_proxy_transfer);
crate::test_for_runtimes!([centrifuge], centrifuge_transfer_with_proxy_borrow);
crate::test_for_runtimes!([centrifuge], centrifuge_transfer_with_proxy_invest);
crate::test_for_runtimes!([centrifuge], centrifuge_x_transfer_with_proxy_transfer);
crate::test_for_runtimes!([centrifuge], centrifuge_x_transfer_with_proxy_borrow);
crate::test_for_runtimes!([centrifuge], centrifuge_x_transfer_with_proxy_invest);
