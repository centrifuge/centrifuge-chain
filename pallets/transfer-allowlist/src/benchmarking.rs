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
use cfg_types::{locations::Location, tokens::CurrencyId};
use codec::EncodeLike;
use frame_benchmarking::*;
use frame_support::traits::{Currency, Get, ReservableCurrency};
use frame_system::RawOrigin;
use scale_info::TypeInfo;
use sp_runtime::traits::{AtLeast32BitUnsigned, Bounded, CheckedAdd, One};

use super::*;
benchmarks! {
	where_clause {
		where
		T: Config<CurrencyId = CurrencyId, Location = Location>,
		<T as frame_system::Config>::AccountId: Into<<T as pallet::Config>::Location>,
		<T as pallet::Config>::Location: From<<T as frame_system::Config>::AccountId> + EncodeLike<<T as pallet::Config>::Location>,
			<T as pallet::Config>::ReserveCurrency: Currency<<T as frame_system::Config>::AccountId> + ReservableCurrency<<T as frame_system::Config>::AccountId>,
		<T as frame_system::Config>::BlockNumber: AtLeast32BitUnsigned + Bounded + TypeInfo

	}

	add_transfer_allowance_no_existing_metadata {
	let (sender, receiver) = set_up_users::<T>();
	}:add_transfer_allowance(RawOrigin::Signed(sender.clone()), CurrencyId::Native, receiver.clone().into())
			verify {
	  assert_eq!(
			Pallet::<T>::get_account_currency_transfer_allowance(
				(sender,
				CurrencyId::Native,
				Location::from(receiver))
			).unwrap(),
			AllowanceDetails {
				allowed_at: T::BlockNumber::zero(),
				blocked_at: T::BlockNumber::max_value(),
			}
		)

	}

	add_transfer_allowance_existing_metadata {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, 200u32.into())?;
	}:add_transfer_allowance(RawOrigin::Signed(sender.clone()), CurrencyId::Native, receiver.clone().into())
	verify {
		assert_eq!(
			Pallet::<T>::get_account_currency_transfer_allowance(
			(sender,
			CurrencyId::Native,
			Location::from(receiver))
			).unwrap(),
			AllowanceDetails {
			allowed_at: <frame_system::Pallet<T>>::block_number() + 200u32.into(),
			blocked_at: T::BlockNumber::max_value(),
			}
		)

	}

	add_allowance_delay_no_existing_metadata {
		let (sender, receiver) = set_up_users::<T>();
	}:add_allowance_delay(RawOrigin::Signed(sender.clone()), CurrencyId::Native, 200u32.into())
	verify {
		assert_eq!(
			Pallet::<T>::get_account_currency_restriction_count_delay(
				sender,
				CurrencyId::Native,
			).unwrap(),
			AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(200u32.into()),
				once_modifiable_after: None
			}
		)
	}
	add_allowance_delay_existing_metadata {
			let (sender, receiver) = set_up_users::<T>();
			Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, receiver.clone().into())?;
	}:add_allowance_delay(RawOrigin::Signed(sender.clone()), CurrencyId::Native, 200u32.into())
		verify {
			assert_eq!(
					Pallet::<T>::get_account_currency_restriction_count_delay(
						  sender,
								CurrencyId::Native,
					).unwrap(),
					AllowanceMetadata {
						  allowance_count: 1,
						  current_delay: Some(200u32.into()),
						  once_modifiable_after: None
					}
			  )
		}


	toggle_allowance_delay_once_future_modifiable {
			let (sender, receiver) = set_up_users::<T>();
		  Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, 1u32.into())?;
	}:toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()), CurrencyId::Native)
		verify {
			assert_eq!(
				Pallet::<T>::get_account_currency_restriction_count_delay(
					sender,
					CurrencyId::Native,
				).unwrap(),
				AllowanceMetadata {
					allowance_count: 0,
					current_delay: Some(1u32.into()),
					once_modifiable_after: Some(2u32.into())
				}
		)
	}

	update_allowance_delay {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, 1u32.into())?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native)?;
		advance_n_blocks::<T>(1u32.into());
	}:update_allowance_delay(RawOrigin::Signed(sender.clone()), CurrencyId::Native, 200u32.into())
	verify {
		assert_eq!(

						Pallet::<T>::get_account_currency_restriction_count_delay(
								sender,
								CurrencyId::Native,
						).unwrap(),
						AllowanceMetadata {
								allowance_count: 0,
								current_delay: Some(200u32.into()),
								once_modifiable_after: None
						}
		)
	}

	purge_allowance_delay_no_remaining_metadata  {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, 1u32.into())?;
		Pallet::<T>::toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native)?;
		advance_n_blocks::<T>(2u32.into());
	}:purge_allowance_delay(RawOrigin::Signed(sender.clone()), CurrencyId::Native)
	verify{
			assert_eq!(

			Pallet::<T>::get_account_currency_restriction_count_delay(
					sender,
					CurrencyId::Native,
				),
			None
		)
	}

	purge_allowance_delay_remaining_metadata {
			let (sender, receiver) = set_up_users::<T>();
			Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, 1u32.into())?;
			Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, receiver.clone().into())?;
			Pallet::<T>::toggle_allowance_delay_once_future_modifiable(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native)?;
			advance_n_blocks::<T>(2u32.into());
	}:purge_allowance_delay(RawOrigin::Signed(sender.clone()), CurrencyId::Native)
	verify{
		assert_eq!(

			Pallet::<T>::get_account_currency_restriction_count_delay(
					sender,
					CurrencyId::Native,
				).unwrap(),
			AllowanceMetadata {
				allowance_count: 1,
				current_delay: None,
				once_modifiable_after: None
			}
		)
	}


	remove_transfer_allowance_delay_present {
		let (sender, receiver) = set_up_users::<T>();
		let delay = T::BlockNumber::one();
		Pallet::<T>::add_allowance_delay(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, delay.clone())?;
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, receiver.clone().into())?;
		advance_n_blocks::<T>(1u32.into());
  }:remove_transfer_allowance(RawOrigin::Signed(sender.clone()), CurrencyId::Native, receiver.clone().into())
	verify {
		assert_eq!(
			Pallet::<T>::get_account_currency_transfer_allowance(
				(sender,
				CurrencyId::Native,
				Location::from(receiver))
			).unwrap(),
			AllowanceDetails {
				allowed_at: T::BlockNumber::one(),
				blocked_at: <frame_system::Pallet<T>>::block_number().checked_add(&delay).expect("Invalid blocked at."),
			}
		)
	}

	remove_transfer_allowance_no_delay {
		let (sender, receiver) = set_up_users::<T>();
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, receiver.clone().into())?;
	}:remove_transfer_allowance(RawOrigin::Signed(sender.clone()), CurrencyId::Native, receiver.clone().into())
	verify {
		assert_eq!(
		Pallet::<T>::get_account_currency_transfer_allowance(
			(sender,
			CurrencyId::Native,
			Location::from(receiver))
		).unwrap(),
		AllowanceDetails {
			allowed_at: T::BlockNumber::zero(),
			blocked_at: <frame_system::Pallet<T>>::block_number(),}
		)
	}

	purge_transfer_allowance_no_remaining_metadata {
			let (sender, receiver) = set_up_users::<T>();
			Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, receiver.clone().into())?;
			Pallet::<T>::remove_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, receiver.clone().into())?;
	}:purge_transfer_allowance(RawOrigin::Signed(sender.clone()), CurrencyId::Native, receiver.clone().into())
	verify {
			assert_eq!(
				Pallet::<T>::get_account_currency_transfer_allowance(
						(sender.clone(),
							CurrencyId::Native,
							Location::from(receiver))
					),
			None
				);
			assert_eq!(

					Pallet::<T>::get_account_currency_restriction_count_delay(
							sender,
							CurrencyId::Native,
					),
					None
			)
	}

	purge_transfer_allowance_remaining_metadata {
		let (sender, receiver) = set_up_users::<T>();
		let receiver_1 = set_up_second_reciever::<T>();
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, receiver.clone().into())?;
		Pallet::<T>::add_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, receiver_1.clone().into())?;
		Pallet::<T>::remove_transfer_allowance(RawOrigin::Signed(sender.clone()).into(), CurrencyId::Native, receiver.clone().into())?;
	}:purge_transfer_allowance(RawOrigin::Signed(sender.clone()), CurrencyId::Native, receiver.clone().into())
	verify {
		assert_eq!(
				Pallet::<T>::get_account_currency_transfer_allowance(
						(sender.clone(),
						CurrencyId::Native,
						Location::from(receiver))
				),
			None
		);

		assert_eq!(

				Pallet::<T>::get_account_currency_restriction_count_delay(
						sender,
						CurrencyId::Native,
				).unwrap(),
				AllowanceMetadata {
						allowance_count: 1,
						current_delay: None,
						once_modifiable_after: None
				}
		)
	}
}

fn set_up_users<T: Config>() -> (T::AccountId, T::AccountId) {
	let sender: T::AccountId = account::<T::AccountId>("Sender", 1, 0);
	let receiver: T::AccountId = account::<T::AccountId>("Receiver", 2, 0);
	T::ReserveCurrency::deposit_creating(
		&sender,
		T::Fees::fee_value(T::AllowanceFeeKey::get()) * 4u32.into(),
	);
	(sender, receiver)
}

fn set_up_second_reciever<T: Config>() -> T::AccountId {
	account::<T::AccountId>("Receiver_1", 3, 0)
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);
