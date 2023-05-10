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

use cfg_test_utils::system::time_travel::advance_n_blocks;
use cfg_traits::fees::Fees;
use codec::EncodeLike;
use frame_benchmarking::*;
use frame_support::traits::{Currency, Get, ReservableCurrency};
use frame_system::RawOrigin;
use scale_info::TypeInfo;
use sp_runtime::traits::{AtLeast32BitUnsigned, Bounded, One};

use super::*;

#[cfg(test)]
fn config_mocks() {
	use crate::mock::Fees;

	Fees::mock_fee_value(|_| 0);
	Fees::mock_fee_to_author(|_, _| Ok(()));
}

benchmarks! {
	where_clause {
		where
		<T as frame_system::Config>::AccountId: Into<<T as pallet::Config>::Location>,
		<T as pallet::Config>::Location: From<<T as frame_system::Config>::AccountId> + EncodeLike<<T as pallet::Config>::Location>,
			<T as pallet::Config>::ReserveCurrency: Currency<<T as frame_system::Config>::AccountId> + ReservableCurrency<<T as frame_system::Config>::AccountId>,
		<T as pallet::Config>::CurrencyId: Default,
		<T as frame_system::Config>::BlockNumber: AtLeast32BitUnsigned + Bounded + TypeInfo

	}

	add_transfer_allowance_no_existing_metadata {
		let (sender, receiver) = set_up_users::<T>();
	}:add_transfer_allowance(RawOrigin::Signed(sender.clone()), T::CurrencyId::default(), receiver.clone().into())


	add_transfer_allowance_existing_metadata {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), 200u32.into())?;
	}:add_transfer_allowance(RawOrigin::Signed(sender.clone()), T::CurrencyId::default(), receiver.clone().into())

	add_allowance_delay_existing_metadata {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), receiver.clone().into())?;
	}:add_allowance_delay(RawOrigin::Signed(sender.clone()), T::CurrencyId::default(), 200u32.into())


	toggle_allowance_delay_once_future_modifiable {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), 1u32.into())?;
	}:toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()), T::CurrencyId::default())

	update_allowance_delay {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), 1u32.into())?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default())?;
		advance_n_blocks::<T>(1u32.into());
	}:update_allowance_delay(RawOrigin::Signed(sender.clone()), T::CurrencyId::default(), 200u32.into())

	purge_allowance_delay_no_remaining_metadata  {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), 1u32.into())?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default())?;
		advance_n_blocks::<T>(2u32.into());
	}:purge_allowance_delay(RawOrigin::Signed(sender.clone()), T::CurrencyId::default())

	remove_transfer_allowance_delay_present {
		let (sender, receiver) = set_up_users::<T>();
		let delay = T::BlockNumber::one();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), delay.clone())?;
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), receiver.clone().into())?;
		advance_n_blocks::<T>(1u32.into());
	}:remove_transfer_allowance(RawOrigin::Signed(sender.clone()), T::CurrencyId::default(), receiver.clone().into())

	purge_transfer_allowance_remaining_metadata {
		let (sender, receiver) = set_up_users::<T>();
		let receiver_1 = set_up_second_receiver::<T>();
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), receiver.clone().into())?;
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), receiver_1.clone().into())?;
		Pallet::<T>::remove_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), T::CurrencyId::default(), receiver.clone().into())?;
	}:purge_transfer_allowance(RawOrigin::Signed(sender.clone()), T::CurrencyId::default(), receiver.clone().into())
}

fn set_up_users<T: Config>() -> (T::AccountId, T::AccountId) {
	#[cfg(test)]
	config_mocks();
	let sender: T::AccountId = account::<T::AccountId>("Sender", 1, 0);
	let receiver: T::AccountId = account::<T::AccountId>("Receiver", 2, 0);
	T::ReserveCurrency::deposit_creating(
		&sender,
		T::Fees::fee_value(T::AllowanceFeeKey::get()) * 4u32.into(),
	);
	(sender, receiver)
}

fn set_up_second_receiver<T: Config>() -> T::AccountId {
	account::<T::AccountId>("Receiver_1", 3, 0)
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);
