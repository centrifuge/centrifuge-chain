// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::{currency_decimals, AccountId, Balance};
use cfg_types::{
	locations::RestrictedTransferLocation,
	tokens::{AssetMetadata, CurrencyId, FilterCurrency},
};
use cumulus_primitives_core::WeightLimit;
use frame_support::{assert_noop, assert_ok, dispatch::RawOrigin};
use runtime_common::remarks::Remark;
use sp_runtime::traits::Zero;

use crate::{
	generic::{
		config::Runtime,
		env::Env,
		envs::runtime_env::RuntimeEnv,
		utils::{
			currency::{cfg, CurrencyInfo, CustomCurrency},
			genesis,
			genesis::Genesis,
			xcm::{account_location, transferable_metadata},
		},
	},
	utils::accounts::Keyring,
};

const TRANSFER_AMOUNT: u32 = 100;

mod local {
	use super::*;

	fn setup<T: Runtime>(filter: FilterCurrency) -> RuntimeEnv<T> {
		let mut env = RuntimeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(TRANSFER_AMOUNT + 10)))
				.storage(),
		);

		env.parachain_state_mut(|| {
			assert_ok!(
				pallet_transfer_allowlist::Pallet::<T>::add_transfer_allowance(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					filter,
					RestrictedTransferLocation::Local(Keyring::Bob.id())
				)
			);

			assert_ok!(pallet_proxy::Pallet::<T>::add_proxy(
				RawOrigin::Signed(Keyring::Alice.into()).into(),
				Keyring::Dave.into(),
				Default::default(),
				Zero::zero(),
			));
		});

		env
	}

	fn people_balances<T: Runtime>() -> (Balance, Balance, Balance) {
		(
			pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.id()),
			pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.id()),
			pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.id()),
		)
	}

	fn process_ok<T: Runtime>(
		env: &mut RuntimeEnv<T>,
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
	) {
		let (pre_transfer_alice, pre_transfer_bob, pre_transfer_charlie) =
			env.parachain_state(|| people_balances::<T>());

		let fee = env.submit_now(who, call).unwrap();
		// NOTE: Only use fee, if submitter is Alice
		let fee = if who != Keyring::Alice { 0 } else { fee };

		let (after_transfer_alice, after_transfer_bob, after_transfer_charlie) =
			env.parachain_state(|| people_balances::<T>());

		assert_eq!(
			after_transfer_alice,
			pre_transfer_alice - fee - cfg(TRANSFER_AMOUNT)
		);
		assert_eq!(after_transfer_bob, pre_transfer_bob + cfg(TRANSFER_AMOUNT));
		assert_eq!(after_transfer_charlie, pre_transfer_charlie);
	}

	fn process_fail<T: Runtime>(
		env: &mut RuntimeEnv<T>,
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
	) {
		let (pre_transfer_alice, pre_transfer_bob, pre_transfer_charlie) =
			env.parachain_state(|| people_balances::<T>());

		let fee = env.submit_now(who, call).unwrap();
		// NOTE: Only use fee, if submitter is Alice
		let fee = if who != Keyring::Alice { 0 } else { fee };

		let (after_transfer_alice, after_transfer_bob, after_transfer_charlie) =
			env.parachain_state(|| people_balances::<T>());

		assert_eq!(after_transfer_alice, pre_transfer_alice - fee);
		assert_eq!(after_transfer_bob, pre_transfer_bob);
		assert_eq!(after_transfer_charlie, pre_transfer_charlie);
	}

	fn validate_ok<T: Runtime>(who: Keyring, call: impl Into<T::RuntimeCallExt> + Clone) {
		let mut env = setup::<T>(FilterCurrency::All);
		process_ok(&mut env, who, call.clone());

		let mut env = setup::<T>(FilterCurrency::Specific(CurrencyId::Native));
		process_ok(&mut env, who, call.clone());
	}

	fn validate_fail<T: Runtime>(who: Keyring, call: impl Into<T::RuntimeCallExt> + Clone) {
		let mut env = setup::<T>(FilterCurrency::All);
		process_fail(&mut env, who, call.clone());

		let mut env = setup::<T>(FilterCurrency::Specific(CurrencyId::Native));
		process_fail(&mut env, who, call.clone());
	}

	fn transfer_to<T: Runtime>(dest: Keyring) -> pallet_balances::Call<T> {
		pallet_balances::Call::transfer_allow_death {
			dest: dest.into(),
			value: cfg(TRANSFER_AMOUNT),
		}
	}

	#[test_runtimes(all)]
	fn transfer_no_restriction<T: Runtime>() {
		let mut env = RuntimeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(TRANSFER_AMOUNT + 10)))
				.storage(),
		);

		process_ok(&mut env, Keyring::Alice, transfer_to(Keyring::Bob));
	}

	#[test_runtimes(all)]
	fn basic_transfer<T: Runtime>() {
		validate_ok::<T>(Keyring::Alice, transfer_to(Keyring::Bob));
		validate_fail::<T>(Keyring::Alice, transfer_to(Keyring::Charlie));
	}

	#[test_runtimes(all)]
	fn proxy_transfer<T: Runtime>() {
		validate_ok::<T>(
			Keyring::Dave,
			pallet_proxy::Call::<T>::proxy {
				real: Keyring::Alice.into(),
				force_proxy_type: None,
				call: Box::new(transfer_to(Keyring::Bob).into()),
			},
		);
		validate_fail::<T>(
			Keyring::Dave,
			pallet_proxy::Call::<T>::proxy {
				real: Keyring::Alice.into(),
				force_proxy_type: None,
				call: Box::new(transfer_to(Keyring::Charlie).into()),
			},
		);
	}

	#[test_runtimes(all)]
	fn batch_proxy_transfer<T: Runtime>() {
		validate_ok::<T>(
			Keyring::Dave,
			pallet_proxy::Call::<T>::proxy {
				real: Keyring::Alice.into(),
				force_proxy_type: None,
				call: Box::new(
					pallet_utility::Call::<T>::batch {
						calls: vec![transfer_to(Keyring::Bob).into()],
					}
					.into(),
				),
			},
		);
		validate_fail::<T>(
			Keyring::Dave,
			pallet_proxy::Call::<T>::proxy {
				real: Keyring::Alice.into(),
				force_proxy_type: None,
				call: Box::new(
					pallet_utility::Call::<T>::batch {
						calls: vec![transfer_to(Keyring::Charlie).into()],
					}
					.into(),
				),
			},
		);
	}

	#[test_runtimes(all)]
	fn batch_transfer<T: Runtime>() {
		validate_ok::<T>(
			Keyring::Alice,
			pallet_utility::Call::<T>::batch {
				calls: vec![transfer_to(Keyring::Bob).into()],
			},
		);
		validate_fail::<T>(
			Keyring::Alice,
			pallet_utility::Call::<T>::batch {
				calls: vec![
					transfer_to(Keyring::Charlie).into(),
					transfer_to(Keyring::Charlie).into(),
					transfer_to(Keyring::Charlie).into(),
				],
			},
		);
	}

	#[test_runtimes(all)]
	fn batch_all_transfer<T: Runtime>() {
		validate_ok::<T>(
			Keyring::Alice,
			pallet_utility::Call::<T>::batch_all {
				calls: vec![transfer_to(Keyring::Bob).into()],
			},
		);
		validate_fail::<T>(
			Keyring::Alice,
			pallet_utility::Call::<T>::batch_all {
				calls: vec![
					transfer_to(Keyring::Charlie).into(),
					transfer_to(Keyring::Charlie).into(),
					transfer_to(Keyring::Charlie).into(),
				],
			},
		);
	}

	#[test_runtimes(all)]
	fn remark_transfer<T: Runtime>() {
		validate_ok::<T>(
			Keyring::Alice,
			pallet_remarks::Call::<T>::remark {
				remarks: vec![Remark::Named(
					"TEST"
						.to_string()
						.as_bytes()
						.to_vec()
						.try_into()
						.expect("Small enough. qed"),
				)]
				.try_into()
				.expect("Small enough. qed."),
				call: Box::new(transfer_to(Keyring::Bob).into()),
			},
		);
		validate_fail::<T>(
			Keyring::Alice,
			pallet_remarks::Call::<T>::remark {
				remarks: vec![Remark::Named(
					"TEST"
						.to_string()
						.as_bytes()
						.to_vec()
						.try_into()
						.expect("Small enough. qed"),
				)]
				.try_into()
				.expect("Small enough. qed."),
				call: Box::new(transfer_to(Keyring::Charlie).into()),
			},
		);
	}
}

mod xcm {
	use super::*;

	#[test_runtimes([centrifuge])]
	fn restrict_xcm_transfer<T: Runtime>() {
		let curr = CustomCurrency(
			CurrencyId::ForeignAsset(1),
			AssetMetadata {
				decimals: 6,
				..transferable_metadata(Some(1000))
			},
		);

		let mut env = RuntimeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1))) // For fees
				.add(genesis::tokens::<T>([(curr.id(), curr.val(3_000))]))
				.add(genesis::assets::<T>([(curr.id(), curr.metadata())]))
				.storage(),
		);

		env.parachain_state_mut(|| {
			assert_ok!(
				pallet_transfer_allowlist::Pallet::<T>::add_transfer_allowance(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					FilterCurrency::Specific(curr.id()),
					RestrictedTransferLocation::Xcm(account_location(
						1,
						Some(1001),
						Keyring::Alice.id()
					)),
				)
			);

			assert_noop!(
				pallet_restricted_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					curr.id(),
					curr.val(1_000),
					account_location(1, Some(1001), Keyring::Bob.id()),
					WeightLimit::Unlimited,
				),
				pallet_transfer_allowlist::Error::<T>::NoAllowanceForDestination
			);

			assert_noop!(
				pallet_restricted_xtokens::Pallet::<T>::transfer(
					RawOrigin::Signed(Keyring::Alice.into()).into(),
					curr.id(),
					curr.val(1_000),
					account_location(1, Some(1001), Keyring::Alice.id()),
					WeightLimit::Unlimited,
				),
				// But it's ok, we do not care about the xcm transaction in this context
				// The xcm transaction is already checked at `cross_transfers.rs`
				orml_xtokens::Error::<T>::XcmExecutionFailed
			);
		});
	}
}

mod domain_address {
	// TODO
}
