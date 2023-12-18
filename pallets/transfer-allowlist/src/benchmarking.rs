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

use cfg_types::tokens::CurrencyId;
use frame_benchmarking::*;
use frame_support::{
	pallet_prelude::Get,
	traits::{fungible::Unbalanced, tokens::Precision, Currency, ReservableCurrency},
};
use frame_system::RawOrigin;
use parity_scale_codec::EncodeLike;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Bounded, CheckedAdd, One},
	Saturating,
};

use super::*;

const BENCHMARK_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

benchmarks! {
	where_clause {
		where
		<T as frame_system::Config>::AccountId: Into<<T as pallet::Config>::Location>,
		<T as pallet::Config>::Location: From<<T as frame_system::Config>::AccountId> + EncodeLike<<T as pallet::Config>::Location>,
			<T as pallet::Config>::ReserveCurrency: Currency<<T as frame_system::Config>::AccountId> + ReservableCurrency<<T as frame_system::Config>::AccountId>,
		T: pallet::Config<CurrencyId = CurrencyId>,
		<T as frame_system::Config>::BlockNumber: AtLeast32BitUnsigned + Bounded + TypeInfo,
		<<T as pallet::Config>::ReserveCurrency as frame_support::traits::fungible::Inspect<<T as frame_system::Config>::AccountId,>>::Balance: From<u64>
	}

	add_transfer_allowance_no_existing_metadata {
		let (sender, receiver) = set_up_users::<T>();
	}:add_transfer_allowance(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID, receiver.clone().into())


	add_transfer_allowance_existing_metadata {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, 200u32.into())?;
	}:add_transfer_allowance(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID, receiver.clone().into())

	add_allowance_delay_no_existing_metadata {
		let (sender, receiver) = set_up_users::<T>();
	}:add_allowance_delay(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID, 200u32.into())

	add_allowance_delay_existing_metadata {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, receiver.clone().into())?;
	}:add_allowance_delay(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID, 200u32.into())


	toggle_allowance_delay_once_future_modifiable {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, 1u32.into())?;
	}:toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID)

	update_allowance_delay {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, 1u32.into())?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID)?;
		let b = frame_system::Pallet::<T>::block_number()
				.checked_add(&1u32.into())
				.expect("Mock block advancement failed.");
		frame_system::Pallet::<T>::set_block_number(b);
	}:update_allowance_delay(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID, 200u32.into())

	purge_allowance_delay_no_remaining_metadata  {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, 1u32.into())?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID)?;
		let b = frame_system::Pallet::<T>::block_number()
				.checked_add(&2u32.into())
				.expect("Mock block advancement failed.");
		frame_system::Pallet::<T>::set_block_number(b);
	}:purge_allowance_delay(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID)

	purge_allowance_delay_remaining_metadata {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, 1u32.into())?;
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, receiver.clone().into())?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID)?;

		let b = frame_system::Pallet::<T>::block_number()
				.checked_add(&2u32.into())
				.expect("Mock block advancement failed.");
		frame_system::Pallet::<T>::set_block_number(b);
	}:purge_allowance_delay(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID)


	remove_transfer_allowance_delay_present {
		let (sender, receiver) = set_up_users::<T>();
		let delay = T::BlockNumber::one();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, delay.clone())?;
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, receiver.clone().into())?;

		let b = frame_system::Pallet::<T>::block_number()
			.checked_add(&1u32.into())
			.expect("Mock block advancement failed.");
		frame_system::Pallet::<T>::set_block_number(b);
	}:remove_transfer_allowance(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID, receiver.clone().into())

	remove_transfer_allowance_no_delay {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, receiver.clone().into())?;
	}:remove_transfer_allowance(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID, receiver.clone().into())

	purge_transfer_allowance_no_remaining_metadata {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, receiver.clone().into())?;
		Pallet::<T>::remove_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, receiver.clone().into())?;
	}:purge_transfer_allowance(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID, receiver.clone().into())

	purge_transfer_allowance_remaining_metadata {
		let (sender, receiver) = set_up_users::<T>();
		let receiver_1 = set_up_second_receiver::<T>();
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, receiver.clone().into())?;
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, receiver_1.clone().into())?;
		Pallet::<T>::remove_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), BENCHMARK_CURRENCY_ID, receiver.clone().into())?;
	}:purge_transfer_allowance(RawOrigin::Signed(sender.clone()), BENCHMARK_CURRENCY_ID, receiver.clone().into())
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

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);
