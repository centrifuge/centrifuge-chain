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

use cfg_types::{fee_keys::FeeKey, locations::Location};
use frame_benchmarking::*;
use frame_support::traits::ReservableCurrency;

use super::*;
use crate::mock::*;

benchmarks! {
	where_clause {
	where T: Config,
		// + pallet_fees::Config<Fees = <T as Config>::Fees>
		// + pallet_balances::Config<Balance = <T as Config>::Balance>,
	// T::Fee
	<T as Config>::CurrencyId: From<CurrencyId> + Into<CurrencyId>,
	<T as Config>::Location: From<Location> + Into<Location>,
	<T as Config>::ReserveCurrency: ReservableCurrency<T::AccountId>
}

	add_allowance {

	}:add_transfer_allowance(RuntimeOrigin::signed(SENDER), CurrencyId::A, ACCOUNT_RECEIVER.into())
	verify {
	  assert_eq!(
				TransferAllowList::get_account_currency_transfer_allowance((
					  SENDER,
					  CurrencyId::A,
					  Location::TestLocal(ACCOUNT_RECEIVER)
				))
					.unwrap(),
				AllowanceDetails {
					  allowed_at: 0u64,
					  blocked_at: u64::MAX,
				}
		  )

  }
  remove_allowance {
	  add_transfer_allowance(RuntimeOrigin::signed(SENDER), CurrencyId::A, ACCOUNT_RECIEVER.into())
  }:remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
	)
	verify {

		  assert_eq!(
				TransferAllowList::get_account_currency_transfer_allowance((
					  SENDER,
					  CurrencyId::A,
					  Location::TestLocal(ACCOUNT_RECEIVER)
				))
					.unwrap(),
				AllowanceDetails {
					  // current block is 50, no delay set
					  allowed_at: 0u64,
					  blocked_at: 50u64,
				}
		  );
	}
	purge_allowance {

	}: {}
	verify {

	}
	add_delay {

	}: {}
	verify {

	}

	update_delay {

	}: {}
	verify {

	}

	toggle_delay_future_modifiable {

	}: {}
	verify {

	}
	purge_delay {

	}: {}
	verify {

	}

}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);
