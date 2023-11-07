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

use cfg_primitives::{AccountId, Balance, CFG};
use cfg_traits::rewards::{AccountRewards, CurrencyGroupChange, DistributedRewards, GroupRewards};
use cfg_types::tokens::CurrencyId;
use development_runtime::{apis::RewardsApi, BlockId};
use frame_support::assert_ok;
use runtime_common::apis::RewardDomain;
use sp_core::{sr25519, Pair};
use sp_runtime::traits::IdentifyAccount;
use tokio::runtime::Handle;

use super::ApiEnv;
use crate::utils::accounts::Keyring;

// #[tokio::test]
// async fn liquidity_rewards_runtime_api_works() {
// 	rewards_runtime_api_works::<development_runtime::LiquidityRewardsBase>(RewardDomain::Liquidity)
// 		.await;
// }

#[tokio::test]
async fn block_rewards_runtime_api_works() {
	rewards_runtime_api_works::<development_runtime::BlockRewardsBase>(RewardDomain::Block).await;
}

type GroupId = u32;

async fn rewards_runtime_api_works<Rewards>(domain: RewardDomain)
where
	Rewards: CurrencyGroupChange<GroupId = GroupId, CurrencyId = CurrencyId>
		+ AccountRewards<AccountId, Balance = Balance, CurrencyId = CurrencyId>
		+ DistributedRewards
		+ GroupRewards<Balance = Balance, GroupId = GroupId>,
{
	let staker = Keyring::Alice.to_account_id();
	let expected_reward = 200 * CFG;
	ApiEnv::new(Handle::current())
		.startup(|| {
			let currencies = vec![(CurrencyId::Native, 1u32)];
			let stake_accounts = vec![(staker.clone(), CurrencyId::Native, 100 * CFG)];
			let rewards = vec![(1, expected_reward)];

			for (currency_id, group_id) in currencies {
				<Rewards as CurrencyGroupChange>::attach_currency(currency_id, group_id)
					.expect("Attaching currency should work");
			}

			for (account_id, currency_id, amount) in stake_accounts {
				<Rewards as AccountRewards<AccountId>>::deposit_stake(
					currency_id,
					&account_id,
					amount,
				)
				.expect("Depositing stake should work");
			}

			for (group_id, amount) in &rewards {
				<Rewards as DistributedRewards>::distribute_reward(*amount, [*group_id])
					.expect("Distributing rewards should work");
			}

			if let RewardDomain::Liquidity = domain {
				/// For the gap mechanism, used by liquidity rewards,
				/// we need another distribution to allow the participant claim
				/// rewards
				for (group_id, amount) in &rewards {
					let res =
						<Rewards as DistributedRewards>::distribute_reward(*amount, [*group_id])
							.expect("Distributing rewards should work");

					res.iter().for_each(|item| {
						item.expect("Rewards distribution error");
					});
				}
			}
		})
		.with_api(|api, latest| {
			let hash = match latest {
				BlockId::Hash(hash) => hash,
				BlockId::Number(n) => todo!("nuno"),
			};

			let currencies = api
				.list_currencies(hash.clone(), domain, staker.clone())
				.expect("There should be staked currencies");
			assert_eq!(currencies.clone().len(), 1);

			let currency_id = currencies[0];

			let reward = api
				.compute_reward(hash, domain, currency_id, staker)
				.unwrap();
			assert_eq!(reward, Some(expected_reward));
		});
}
