// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use super::*;
use mock::*;

const TEST_ACCOUNT: AccountId = AccountId::new([1; 32]);

#[test]
fn test_fees_and_tip_split() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			const FEE: u64 = 10;
			const TIP: u64 = 20;

			let fee = Balances::issue(FEE);
			let tip = Balances::issue(TIP);

			assert_eq!(Balances::free_balance(Treasury::account_id()), 0);
			assert_eq!(Balances::free_balance(TEST_ACCOUNT), 0);

			DealWithFees::on_unbalanceds(vec![fee, tip].into_iter());

			assert_eq!(
				Balances::free_balance(Treasury::account_id()),
				TREASURY_FEE_RATIO * FEE
			);
			assert_eq!(
				Balances::free_balance(TEST_ACCOUNT),
				TIP + (Perbill::one() - TREASURY_FEE_RATIO) * FEE
			);
		});
}
