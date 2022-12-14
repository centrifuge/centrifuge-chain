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

use cfg_primitives::{AccountId, CFG};
use cfg_traits::rewards::{AccountRewards, CurrencyGroupChange, DistributedRewards};
use development_runtime::apis::RewardsApi;
use frame_support::assert_ok;
use sp_core::{sr25519, Pair};
use sp_runtime::traits::IdentifyAccount;
use tokio::runtime::Handle;

use super::ApiEnv;

#[tokio::test]
async fn test() {
	ApiEnv::new(Handle::current())
		.startup(|| {
			let currencies = vec![(
				(
					development_runtime::RewardDomain::Block,
					development_runtime::CurrencyId::Native,
				),
				1,
			)];
			let stake_accounts = vec![(
				sp_runtime::AccountId32::from(
					<sr25519::Pair as sp_core::Pair>::from_string("//Alice", None)
						.unwrap()
						.public()
						.into_account(),
				),
				(
					development_runtime::RewardDomain::Block,
					development_runtime::CurrencyId::Native,
				),
				100 * CFG,
			)];
			let rewards = vec![(1, 200 * CFG)];

			for ((domain_id, currency_id), group_id) in currencies {
				<development_runtime::Rewards as CurrencyGroupChange>::attach_currency(
					(domain_id, currency_id),
					group_id,
				)
				.unwrap();
			}

			for (account_id, (domain_id, currency_id), amount) in stake_accounts {
				<development_runtime::Rewards as AccountRewards<AccountId>>::deposit_stake(
					(domain_id, currency_id),
					&account_id,
					amount,
				)
				.unwrap();
			}

			for (group_id, amount) in rewards {
				<development_runtime::Rewards as DistributedRewards>::distribute_reward(
					amount,
					[group_id],
				)
				.unwrap();
			}
		})
		.with_api(|api, latest| {
			let account_id = sp_runtime::AccountId32::from(
				<sr25519::Pair as sp_core::Pair>::from_string("//Alice", None)
					.unwrap()
					.public()
					.into_account(),
			);

			let currencies = api.list_currencies(&latest, account_id.clone()).unwrap();
			assert_eq!(currencies.clone().len(), 1);

			let currency_id = currencies[0];

			let reward = api
				.compute_reward(&latest, currency_id, account_id)
				.unwrap();
			assert_eq!(reward, Some(200 * CFG));
		});
}
