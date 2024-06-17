use cfg_types::tokens::{AssetMetadata, CurrencyId};
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use sp_runtime::{traits::StaticLookup, DispatchResult};
use staging_xcm::v4::WeightLimit;

use crate::{
	generic::{
		config::Runtime,
		env::Env,
		envs::runtime_env::RuntimeEnv,
		utils::{
			currency::{cfg, CurrencyInfo, CustomCurrency},
			genesis::{self, Genesis},
			xcm::{account_location, transferable_metadata},
		},
	},
	utils::accounts::Keyring,
};

const FROM: Keyring = Keyring::Charlie;
const PROXY: Keyring = Keyring::Alice;
const TO: Keyring = Keyring::Bob;

enum TransferKind {
	Local,
	Xcm,
}

fn run_test<T: Runtime>(proxy_type: T::ProxyType, transfer_kind: TransferKind) -> DispatchResult {
	let para_id = 1234;
	let curr = CustomCurrency(
		CurrencyId::ForeignAsset(1),
		AssetMetadata {
			decimals: 6,
			..transferable_metadata(Some(para_id))
		},
	);

	let mut env = RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(cfg(1))) // For fees
			.add(genesis::tokens::<T>(vec![(curr.id(), curr.val(1000))]))
			.add(genesis::assets::<T>(vec![(curr.id(), curr.metadata())]))
			.storage(),
	);

	let call = match transfer_kind {
		TransferKind::Local => pallet_restricted_tokens::Call::transfer {
			currency_id: curr.id(),
			amount: curr.val(100),
			dest: T::Lookup::unlookup(TO.id()),
		}
		.into(),
		TransferKind::Xcm => pallet_restricted_xtokens::Call::transfer {
			currency_id: curr.id(),
			amount: curr.val(100),
			dest: account_location(1, Some(para_id), TO.id()),
			dest_weight_limit: WeightLimit::Unlimited,
		}
		.into(),
	};

	env.parachain_state_mut(|| {
		// Register PROXY as proxy of FROM
		assert_ok!(pallet_proxy::Pallet::<T>::add_proxy(
			RawOrigin::Signed(FROM.id()).into(),
			T::Lookup::unlookup(PROXY.id()),
			proxy_type,
			0,
		));

		// Acts as FROM using PROXY
		assert_ok!(pallet_proxy::Pallet::<T>::proxy(
			RawOrigin::Signed(PROXY.id()).into(),
			T::Lookup::unlookup(FROM.id()),
			None,
			Box::new(call),
		));
	});

	env.find_event(|e| match e {
		pallet_proxy::Event::<T>::ProxyExecuted { result } => {
			if result == Err(orml_xtokens::Error::<T>::XcmExecutionFailed.into()) {
				// We have not configured XCM, so if we reach the sending phase though xcm we
				// can assert that proxy was filtered correctly.
				Some(Ok(()))
			} else {
				Some(result)
			}
		}
		_ => None,
	})
	.unwrap()
}

#[test_runtimes([development])]
fn development_transfer_with_proxy_transfer<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert_ok!(run_test::<T>(
		development_runtime::ProxyType::Transfer,
		TransferKind::Local
	));
}

#[test_runtimes([development])]
fn development_transfer_with_proxy_borrow<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(development_runtime::ProxyType::Borrow, TransferKind::Local),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([development])]
fn development_transfer_with_proxy_invest<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(development_runtime::ProxyType::Invest, TransferKind::Local),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([development])]
fn development_x_transfer_with_proxy_transfer<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert_ok!(run_test::<T>(
		development_runtime::ProxyType::Transfer,
		TransferKind::Xcm
	));
}

#[test_runtimes([development])]
fn development_x_transfer_with_proxy_borrow<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(development_runtime::ProxyType::Borrow, TransferKind::Xcm),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([development])]
fn development_x_transfer_with_proxy_invest<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = development_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(development_runtime::ProxyType::Invest, TransferKind::Xcm),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([altair])]
fn altair_transfer_with_proxy_transfer<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert_ok!(run_test::<T>(
		altair_runtime::ProxyType::Transfer,
		TransferKind::Local
	));
}

#[test_runtimes([altair])]
fn altair_transfer_with_proxy_borrow<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(altair_runtime::ProxyType::Borrow, TransferKind::Local),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([altair])]
fn altair_transfer_with_proxy_invest<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(altair_runtime::ProxyType::Invest, TransferKind::Local),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([altair])]
fn altair_x_transfer_with_proxy_transfer<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert_ok!(run_test::<T>(
		altair_runtime::ProxyType::Transfer,
		TransferKind::Xcm
	));
}

#[test_runtimes([altair])]
fn altair_x_transfer_with_proxy_borrow<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(altair_runtime::ProxyType::Borrow, TransferKind::Xcm),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([altair])]
fn altair_x_transfer_with_proxy_invest<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = altair_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(altair_runtime::ProxyType::Invest, TransferKind::Xcm),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([centrifuge])]
fn centrifuge_transfer_with_proxy_transfer<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert_ok!(run_test::<T>(
		centrifuge_runtime::ProxyType::Transfer,
		TransferKind::Local
	));
}

#[test_runtimes([centrifuge])]
fn centrifuge_transfer_with_proxy_borrow<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(centrifuge_runtime::ProxyType::Borrow, TransferKind::Local),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([centrifuge])]
fn centrifuge_transfer_with_proxy_invest<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(centrifuge_runtime::ProxyType::Invest, TransferKind::Local),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([centrifuge])]
fn centrifuge_x_transfer_with_proxy_transfer<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert_ok!(run_test::<T>(
		centrifuge_runtime::ProxyType::Transfer,
		TransferKind::Xcm
	));
}

#[test_runtimes([centrifuge])]
fn centrifuge_x_transfer_with_proxy_borrow<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(centrifuge_runtime::ProxyType::Borrow, TransferKind::Xcm),
		frame_system::Error::<T>::CallFiltered,
	);
}

#[test_runtimes([centrifuge])]
fn centrifuge_x_transfer_with_proxy_invest<T: Runtime>()
where
	T: pallet_proxy::Config<ProxyType = centrifuge_runtime::ProxyType>,
{
	assert_err!(
		run_test::<T>(centrifuge_runtime::ProxyType::Invest, TransferKind::Xcm),
		frame_system::Error::<T>::CallFiltered,
	);
}
