// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

#![cfg(feature = "runtime-benchmarks")]

use cfg_types::tokens::{CurrencyId, FilterCurrency};
use frame_benchmarking::{account, v2::*};
use frame_support::{
	pallet_prelude::Get,
	traits::{fungible::Unbalanced, tokens::Precision, Currency, ReservableCurrency},
};
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use sp_core::crypto::AccountId32;
use sp_runtime::{
	traits::{CheckedAdd, One},
	Saturating,
};

use super::*;

const BENCHMARK_CURRENCY_ID: FilterCurrency = FilterCurrency::Specific(CurrencyId::ForeignAsset(1));

#[benchmarks(
where
	T: Config<CurrencyId = FilterCurrency>,
	<T as frame_system::Config>::AccountId: Into<AccountId32>,
	T::Location: From<<T as frame_system::Config>::AccountId>,
	T::ReserveCurrency: Currency<<T as frame_system::Config>::AccountId> + ReservableCurrency<<T as frame_system::Config>::AccountId>,
	BlockNumberFor<T>: One,
	<<T as Config>::ReserveCurrency as frame_support::traits::fungible::Inspect<<T as frame_system::Config>::AccountId,>>::Balance: From<u64>
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn add_transfer_allowance_no_existing_metadata() -> Result<(), BenchmarkError> {
		let (sender, receiver) = set_up_users::<T>();

		#[extrinsic_call]
		add_transfer_allowance(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver),
		);

		Ok(())
	}

	#[benchmark]
	fn add_transfer_allowance_existing_metadata() -> Result<(), BenchmarkError> {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			200u32.into(),
		)?;

		#[extrinsic_call]
		add_transfer_allowance(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver),
		);

		Ok(())
	}
	#[benchmark]
	fn add_allowance_delay_no_existing_metadata() -> Result<(), BenchmarkError> {
		let (sender, _) = set_up_users::<T>();

		#[extrinsic_call]
		add_allowance_delay(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
			200u32.into(),
		);

		Ok(())
	}
	#[benchmark]
	fn add_allowance_delay_existing_metadata() -> Result<(), BenchmarkError> {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_transfer_allowance(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver),
		)?;

		#[extrinsic_call]
		add_allowance_delay(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
			200u32.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn toggle_allowance_delay_once_future_modifiable() -> Result<(), BenchmarkError> {
		let (sender, _) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			1u32.into(),
		)?;

		#[extrinsic_call]
		toggle_allowance_delay_once_future_modifiable(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
		);

		Ok(())
	}

	#[benchmark]
	fn update_allowance_delay() -> Result<(), BenchmarkError> {
		let (sender, _) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			1u32.into(),
		)?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
		)?;
		let b = frame_system::Pallet::<T>::block_number()
			.checked_add(&1u32.into())
			.expect("Mock block advancement failed.");
		frame_system::Pallet::<T>::set_block_number(b);

		#[extrinsic_call]
		update_allowance_delay(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
			200u32.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn purge_allowance_delay_no_remaining_metadata() -> Result<(), BenchmarkError> {
		let (sender, _) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			1u32.into(),
		)?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
		)?;
		let b = frame_system::Pallet::<T>::block_number()
			.checked_add(&2u32.into())
			.expect("Mock block advancement failed.");
		frame_system::Pallet::<T>::set_block_number(b);

		#[extrinsic_call]
		purge_allowance_delay(RawOrigin::Signed(sender), BENCHMARK_CURRENCY_ID);

		Ok(())
	}

	#[benchmark]
	fn purge_allowance_delay_remaining_metadata() -> Result<(), BenchmarkError> {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			1u32.into(),
		)?;
		Pallet::<T>::add_transfer_allowance(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver),
		)?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
		)?;

		let b = frame_system::Pallet::<T>::block_number()
			.checked_add(&2u32.into())
			.expect("Mock block advancement failed.");
		frame_system::Pallet::<T>::set_block_number(b);

		#[extrinsic_call]
		purge_allowance_delay(RawOrigin::Signed(sender), BENCHMARK_CURRENCY_ID);

		Ok(())
	}

	#[benchmark]
	fn remove_transfer_allowance_delay_present() -> Result<(), BenchmarkError> {
		let (sender, receiver) = set_up_users::<T>();
		let delay = BlockNumberFor::<T>::one();
		Pallet::<T>::add_allowance_delay(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			delay.clone(),
		)?;
		Pallet::<T>::add_transfer_allowance(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver.clone()),
		)?;

		let b = frame_system::Pallet::<T>::block_number()
			.checked_add(&1u32.into())
			.expect("Mock block advancement failed.");
		frame_system::Pallet::<T>::set_block_number(b);

		#[extrinsic_call]
		remove_transfer_allowance(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver),
		);

		Ok(())
	}

	#[benchmark]
	fn remove_transfer_allowance_no_delay() -> Result<(), BenchmarkError> {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_transfer_allowance(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver.clone()),
		)?;

		#[extrinsic_call]
		remove_transfer_allowance(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver),
		);

		Ok(())
	}

	#[benchmark]
	fn purge_transfer_allowance_no_remaining_metadata() -> Result<(), BenchmarkError> {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_transfer_allowance(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver.clone()),
		)?;
		Pallet::<T>::remove_transfer_allowance(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver.clone()),
		)?;

		#[extrinsic_call]
		purge_transfer_allowance(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver.clone()),
		);

		Ok(())
	}

	#[benchmark]
	fn purge_transfer_allowance_remaining_metadata() -> Result<(), BenchmarkError> {
		let (sender, receiver) = set_up_users::<T>();
		let receiver_1 = set_up_second_receiver::<T>();
		Pallet::<T>::add_transfer_allowance(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver.clone()),
		)?;
		Pallet::<T>::add_transfer_allowance(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver_1.clone()),
		)?;
		Pallet::<T>::remove_transfer_allowance(
			RawOrigin::Signed(sender.clone()).into(),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver.clone()),
		)?;

		#[extrinsic_call]
		purge_transfer_allowance(
			RawOrigin::Signed(sender),
			BENCHMARK_CURRENCY_ID,
			T::Location::from(receiver),
		);

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}

fn set_up_users<T: Config>() -> (T::AccountId, T::AccountId)
where
	<<T as Config>::ReserveCurrency as frame_support::traits::fungible::Inspect<
		<T as frame_system::Config>::AccountId,
	>>::Balance: From<u64>,
{
	let sender: T::AccountId = account::<T::AccountId>("Sender", 1, 0);
	let receiver: T::AccountId = account::<T::AccountId>("Receiver", 2, 0);

	T::ReserveCurrency::increase_balance(
		&sender,
		T::Deposit::get().saturating_mul(10.into()),
		Precision::BestEffort,
	)
	.expect("sender account balance can be increased");

	(sender, receiver)
}

fn set_up_second_receiver<T: Config>() -> T::AccountId {
	account::<T::AccountId>("Receiver_1", 3, 0)
}
