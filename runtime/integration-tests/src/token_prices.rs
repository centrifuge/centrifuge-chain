use runtime_common::{Amount, SECONDS_PER_DAY};
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
use crate::utils::{
	create_default_pool, get_admin, get_tranche_prices, into_signed, issue_loan, next_block,
	pass_time, start_env,
};
use crate::utils::{get_signed, DefaultMinEpochTime, Loans, Pools, System, Timestamp};

#[test]
fn token_price_stays_zero() {
	start_env().execute_with(|| {
		let pool_id = 0;
		create_default_pool(pool_id);

		Pools::update_invest_order(get_signed(0), pool_id, 0, 500).unwrap();
		Pools::update_invest_order(get_signed(1), pool_id, 0, 500).unwrap();

		pass_time(DefaultMinEpochTime::get());

		Loans::update_nav(into_signed(get_admin()), pool_id).unwrap();
		Pools::close_epoch(into_signed(get_admin()), pool_id).unwrap();

		next_block();
		let prices = get_tranche_prices(pool_id);
		let loan_id = issue_loan(pool_id, 1000);
		Loans::borrow(
			into_signed(get_admin()),
			pool_id,
			loan_id,
			Amount::from_inner(900),
		)
		.unwrap();

		pass_time(20 * SECONDS_PER_DAY);

		Loans::update_nav(into_signed(get_admin()), pool_id).unwrap();
		Pools::close_epoch(into_signed(get_admin()), pool_id).unwrap();

		let block = System::block_number();
		let time = Timestamp::now();
		let prices_after = get_tranche_prices(pool_id);

		let x = 0;
	})
}
