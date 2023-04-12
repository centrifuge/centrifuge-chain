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

benchmarks! {
	where_clause {
	where T:   Config + pallet_fees::Config<Fees = <T as Config>::Fees> + pallet_balances::Config<Balance = <T as Config>::Balance>, <T as Config>::CurrencyId: From<CurrencyId> + Into<CurrencyId>, <T as Config>::Location: From<Location> + Into<Location>, <T as Config>::ReserveCurrency: ReservableCurrency<T::AccountId>
}

	add_allowance {

	}
  remove_allowance {

  }
	purge_allowance {

	}
	add_delay {

	}

	update_delay {

	}

	toggle_delay_future_modifiable {

	}
	purge_delay {

	}

}
