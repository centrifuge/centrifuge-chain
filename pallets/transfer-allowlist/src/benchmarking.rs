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

use cfg_types::{locations::Location, tokens::CurrencyId};
use codec::EncodeLike;
use frame_benchmarking::*;
use frame_support::traits::{Currency, ReservableCurrency};
use frame_system::RawOrigin;
use sp_runtime::traits::{AtLeast32BitUnsigned, Bounded};

use super::*;
benchmarks! {
	where_clause {
	  where
		T: Config<CurrencyId = CurrencyId, Location = Location>,
	<T as frame_system::Config>::AccountId: Into<<T as pallet::Config>::Location> + AtLeast32BitUnsigned,
	  <T as pallet::Config>::Location: From<<T as frame_system::Config>::AccountId> + EncodeLike<<T as pallet::Config>::Location>,
	  <T as pallet::Config>::ReserveCurrency: Currency<<T as frame_system::Config>::AccountId> + ReservableCurrency<<T as frame_system::Config>::AccountId>,
	  <T as frame_system::Config>::BlockNumber: AtLeast32BitUnsigned + Bounded

}

	add_transfer_allowance {
	  let sender: T::AccountId = account::<T::AccountId>("Sender", 1,0);
	  let receiver: T::AccountId = account::<T::AccountId>("Receiver", 2,0);
		  T::ReserveCurrency::deposit_creating(&sender, 100u32.into());

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

  remove_transfer_allowance {
		let sender: T::AccountId = account::<T::AccountId>("Sender", 1,0);
		let receiver: T::AccountId = account::<T::AccountId>("Receiver", 2,0);
		T::ReserveCurrency::deposit_creating(&sender, 100u32.into());
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
					  blocked_at: <frame_system::Pallet<T>>::block_number(),
				}
		  )
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);
