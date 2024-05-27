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

mod cfg {
	use cfg_primitives::{currency_decimals, Balance};
	use cfg_types::{
		locations::RestrictedTransferLocation,
		tokens::{CurrencyId, FilterCurrency},
	};
	use frame_support::{assert_ok, dispatch::RawOrigin};
	use runtime_common::remarks::Remark;
	use sp_runtime::traits::Zero;

	use crate::{
		generic::{
			config::Runtime,
			env::Env,
			envs::runtime_env::RuntimeEnv,
			utils::{genesis, genesis::Genesis},
		},
		utils::accounts::Keyring,
	};

	const TRANSFER_AMOUNT: Balance = 100;

	pub fn decimals(decimals: u32) -> Balance {
		10u128.saturating_pow(decimals)
	}
	pub fn cfg(amount: Balance) -> Balance {
		amount * decimals(currency_decimals::NATIVE)
	}

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
					RestrictedTransferLocation::Local(Keyring::Bob.to_account_id())
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

	fn validate_fail<T: Runtime>(who: Keyring, call: impl Into<T::RuntimeCallExt> + Clone) {
		// With FilterCurrencyAll
		{
			let mut env = setup::<T>(FilterCurrency::All);

			let (pre_transfer_alice, pre_transfer_bob, pre_transfer_charlie) =
				env.parachain_state(|| {
					// NOTE: The para-id is not relevant here
					(
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(
							&Keyring::Charlie.to_account_id(),
						),
					)
				});

			let fee = env.submit_now(who, call.clone()).unwrap();
			// NOTE: Only use fee, if submitter is Alice
			let fee = if who != Keyring::Alice { 0 } else { fee };

			env.parachain_state(|| {
				let after_transfer_alice =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id());
				let after_transfer_bob =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id());
				let after_transfer_charlie =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id());

				assert_eq!(after_transfer_alice, pre_transfer_alice - fee);
				assert_eq!(after_transfer_bob, pre_transfer_bob);
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}

		// With FilterCurrency::Specific(CurrencyId::Native)
		{
			let mut env = setup::<T>(FilterCurrency::Specific(CurrencyId::Native));

			let (pre_transfer_alice, pre_transfer_bob, pre_transfer_charlie) =
				env.parachain_state(|| {
					// NOTE: The para-id is not relevant here
					(
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(
							&Keyring::Charlie.to_account_id(),
						),
					)
				});

			let fee = env.submit_now(who, call).unwrap();
			// NOTE: Only use fee, if submitter is Alice
			let fee = if who != Keyring::Alice { 0 } else { fee };

			env.parachain_state(|| {
				let after_transfer_alice =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id());
				let after_transfer_bob =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id());
				let after_transfer_charlie =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id());

				assert_eq!(after_transfer_alice, pre_transfer_alice - fee);
				assert_eq!(after_transfer_bob, pre_transfer_bob);
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}
	}

	fn validate_ok<T: Runtime>(who: Keyring, call: impl Into<T::RuntimeCallExt> + Clone) {
		// With FilterCurrencyAll
		{
			let mut env = setup::<T>(FilterCurrency::All);

			let (pre_transfer_alice, pre_transfer_bob, pre_transfer_charlie) =
				env.parachain_state(|| {
					// NOTE: The para-id is not relevant here
					(
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(
							&Keyring::Charlie.to_account_id(),
						),
					)
				});

			let fee = env.submit_now(who, call.clone()).unwrap();

			// NOTE: Only use fee, if submitter is Alice
			let fee = if who != Keyring::Alice { 0 } else { fee };

			env.parachain_state(|| {
				let after_transfer_alice =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id());
				let after_transfer_bob =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id());
				let after_transfer_charlie =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id());

				assert_eq!(
					after_transfer_alice,
					pre_transfer_alice - fee - cfg(TRANSFER_AMOUNT)
				);
				assert_eq!(after_transfer_bob, pre_transfer_bob + cfg(TRANSFER_AMOUNT));
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}

		// With FilterCurrency::Specific(CurrencyId::Native)
		{
			let mut env = setup::<T>(FilterCurrency::Specific(CurrencyId::Native));

			let (pre_transfer_alice, pre_transfer_bob, pre_transfer_charlie) =
				env.parachain_state(|| {
					// NOTE: The para-id is not relevant here
					(
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id()),
						pallet_balances::Pallet::<T>::free_balance(
							&Keyring::Charlie.to_account_id(),
						),
					)
				});

			let fee = env.submit_now(who, call).unwrap();
			// NOTE: Only use fee, if submitter is Alice
			let fee = if who != Keyring::Alice { 0 } else { fee };

			env.parachain_state(|| {
				let after_transfer_alice =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id());
				let after_transfer_bob =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id());
				let after_transfer_charlie =
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id());

				assert_eq!(
					after_transfer_alice,
					pre_transfer_alice - fee - cfg(TRANSFER_AMOUNT)
				);
				assert_eq!(after_transfer_bob, pre_transfer_bob + cfg(TRANSFER_AMOUNT));
				assert_eq!(after_transfer_charlie, pre_transfer_charlie);
			});
		}
	}

	fn transfer_ok<T: Runtime>() -> pallet_balances::Call<T> {
		pallet_balances::Call::<T>::transfer_allow_death {
			dest: Keyring::Bob.into(),
			value: cfg(TRANSFER_AMOUNT),
		}
	}

	fn transfer_fail<T: Runtime>() -> pallet_balances::Call<T> {
		pallet_balances::Call::<T>::transfer_allow_death {
			dest: Keyring::Charlie.into(),
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

		let (pre_transfer_alice, pre_transfer_bob, pre_transfer_charlie) =
			env.parachain_state(|| {
				// NOTE: The para-id is not relevant here
				(
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id()),
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id()),
					pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id()),
				)
			});

		let fee = env.submit_now(Keyring::Alice, transfer_ok::<T>()).unwrap();

		env.parachain_state(|| {
			let after_transfer_alice =
				pallet_balances::Pallet::<T>::free_balance(&Keyring::Alice.to_account_id());
			let after_transfer_bob =
				pallet_balances::Pallet::<T>::free_balance(&Keyring::Bob.to_account_id());
			let after_transfer_charlie =
				pallet_balances::Pallet::<T>::free_balance(&Keyring::Charlie.to_account_id());

			assert_eq!(
				after_transfer_alice,
				pre_transfer_alice - fee - cfg(TRANSFER_AMOUNT)
			);
			assert_eq!(after_transfer_bob, pre_transfer_bob + cfg(TRANSFER_AMOUNT));
			assert_eq!(after_transfer_charlie, pre_transfer_charlie);
		});
	}

	#[test_runtimes(all)]
	fn basic_transfer<T: Runtime>() {
		validate_ok::<T>(Keyring::Alice, transfer_ok::<T>());
		validate_fail::<T>(Keyring::Alice, transfer_fail::<T>());
	}

	#[test_runtimes(all)]
	fn proxy_transfer<T: Runtime>() {
		validate_ok::<T>(
			Keyring::Dave,
			pallet_proxy::Call::<T>::proxy {
				real: Keyring::Alice.into(),
				force_proxy_type: None,
				call: Box::new(transfer_ok::<T>().into()),
			},
		);
		validate_fail::<T>(
			Keyring::Dave,
			pallet_proxy::Call::<T>::proxy {
				real: Keyring::Alice.into(),
				force_proxy_type: None,
				call: Box::new(transfer_fail::<T>().into()),
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
						calls: vec![transfer_ok::<T>().into()],
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
						calls: vec![transfer_fail::<T>().into()],
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
				calls: vec![transfer_ok::<T>().into()],
			},
		);
		validate_fail::<T>(
			Keyring::Alice,
			pallet_utility::Call::<T>::batch {
				calls: vec![
					transfer_fail::<T>().into(),
					transfer_fail::<T>().into(),
					transfer_fail::<T>().into(),
				],
			},
		);
	}

	#[test_runtimes(all)]
	fn batch_all_transfer<T: Runtime>() {
		validate_ok::<T>(
			Keyring::Alice,
			pallet_utility::Call::<T>::batch_all {
				calls: vec![transfer_ok::<T>().into()],
			},
		);
		validate_fail::<T>(
			Keyring::Alice,
			pallet_utility::Call::<T>::batch_all {
				calls: vec![
					transfer_fail::<T>().into(),
					transfer_fail::<T>().into(),
					transfer_fail::<T>().into(),
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
				call: Box::new(transfer_ok::<T>().into()),
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
				call: Box::new(transfer_fail::<T>().into()),
			},
		);
	}
}
