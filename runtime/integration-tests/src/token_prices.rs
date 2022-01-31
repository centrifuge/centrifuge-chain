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
use crate::utils::{create_default_pool, get_admin, into_signed, pass_time, start_env};
use crate::utils::{get_signed, DefaultMinEpochTime, Loans, Pools};

#[test]
fn token_price_stays_zero() {
	start_env().execute_with(|| {
		create_default_pool(0).unwrap();

		Pools::update_invest_order(get_signed(0), 0, 0, 500).unwrap();
		Pools::update_invest_order(get_signed(1), 0, 0, 500).unwrap();

		pass_time(DefaultMinEpochTime::get());

		Loans::update_nav(into_signed(get_admin()), 0).unwrap();
		Pools::close_epoch(into_signed(get_admin()), 0).unwrap();
	})
}
