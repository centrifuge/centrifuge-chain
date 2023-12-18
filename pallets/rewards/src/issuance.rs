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

use cfg_traits::rewards::RewardIssuance;
use frame_support::traits::{fungibles::Mutate, tokens::Preservation};
use parity_scale_codec::{Decode, Encode};
use sp_runtime::{traits::Get, DispatchResult};
use sp_std::marker::PhantomData;

/// Enables rewarding out of thin air, e.g. via minting.
pub struct MintReward<AccountId, Balance, CurrencyId, Currency>(
	PhantomData<(AccountId, Balance, CurrencyId, Currency)>,
);

impl<AccountId, Balance, CurrencyId, Currency> RewardIssuance
	for MintReward<AccountId, Balance, CurrencyId, Currency>
where
	AccountId: Encode + Decode,
	Currency: Mutate<AccountId, AssetId = CurrencyId, Balance = Balance>,
{
	type AccountId = AccountId;
	type Balance = Balance;
	type CurrencyId = CurrencyId;

	fn issue_reward(
		currency_id: Self::CurrencyId,
		beneficiary: &Self::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		Currency::mint_into(currency_id, beneficiary, amount).map(|_| ())
	}
}

/// Enables rewarding from an account address, e.g. the Treasury.
pub struct TransferReward<AccountId, Balance, CurrencyId, Currency, SourceAddress>(
	PhantomData<(AccountId, Balance, CurrencyId, Currency, SourceAddress)>,
);

impl<AccountId, Balance, CurrencyId, Currency, SourceAddress> RewardIssuance
	for TransferReward<AccountId, Balance, CurrencyId, Currency, SourceAddress>
where
	AccountId: Encode + Decode,
	Currency: Mutate<AccountId, AssetId = CurrencyId, Balance = Balance>,
	SourceAddress: Get<AccountId>,
{
	type AccountId = AccountId;
	type Balance = Balance;
	type CurrencyId = CurrencyId;

	fn issue_reward(
		currency_id: Self::CurrencyId,
		beneficiary: &Self::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		Currency::transfer(
			currency_id,
			&SourceAddress::get(),
			beneficiary,
			amount,
			Preservation::Protect,
		)
		.map(|_| ())
	}
}

#[cfg(test)]
mod tests {
	use frame_support::{assert_err, assert_ok};
	use orml_traits::MultiCurrency;

	use super::*;
	use crate::mock::{
		new_test_ext, CurrencyId, Runtime, Tokens, REWARD_SOURCE, USER_A as BENEFICIARY,
		USER_INITIAL_BALANCE,
	};

	type Balance = u64;
	type AccountId = u64;

	frame_support::parameter_types! {
		pub const Source: u64 = REWARD_SOURCE;
		pub const InsufficientFunds: u64 = REWARD_SOURCE - 1;
	}

	const CURRENCY_ID: CurrencyId = CurrencyId::Reward;
	const REWARD_AMOUNT: Balance = 1234;

	#[test]
	fn mint_reward_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(Tokens::free_balance(CURRENCY_ID, &BENEFICIARY), 0);
			assert_eq!(Tokens::total_issuance(CURRENCY_ID), USER_INITIAL_BALANCE);

			assert_ok!(
				MintReward::<AccountId, Balance, CurrencyId, Tokens>::issue_reward(
					CURRENCY_ID,
					&BENEFICIARY,
					REWARD_AMOUNT,
				),
			);
			assert_eq!(
				Tokens::free_balance(CURRENCY_ID, &BENEFICIARY),
				REWARD_AMOUNT
			);
			assert_eq!(
				Tokens::total_issuance(CURRENCY_ID),
				USER_INITIAL_BALANCE + REWARD_AMOUNT
			);
		})
	}

	#[test]
	fn transfer_reward_works() {
		new_test_ext().execute_with(|| {
			let issuance_before = Tokens::total_issuance(CURRENCY_ID);
			assert_eq!(Tokens::free_balance(CURRENCY_ID, &BENEFICIARY), 0);
			assert_eq!(
				Tokens::free_balance(CURRENCY_ID, &REWARD_SOURCE),
				USER_INITIAL_BALANCE
			);

			assert_ok!(TransferReward::<
				AccountId,
				Balance,
				CurrencyId,
				Tokens,
				Source,
			>::issue_reward(CURRENCY_ID, &BENEFICIARY, REWARD_AMOUNT));
			assert_eq!(
				Tokens::free_balance(CURRENCY_ID, &BENEFICIARY),
				REWARD_AMOUNT
			);
			assert_eq!(
				Tokens::free_balance(CURRENCY_ID, &REWARD_SOURCE),
				USER_INITIAL_BALANCE - REWARD_AMOUNT
			);
			assert_eq!(Tokens::total_issuance(CURRENCY_ID), issuance_before);
		})
	}

	#[test]
	fn transfer_reward_missing_funds_throws() {
		new_test_ext().execute_with(|| {
			assert_err!(TransferReward::<
				AccountId,
				Balance,
				CurrencyId,
				Tokens,
				InsufficientFunds,
			>::issue_reward(CURRENCY_ID, &BENEFICIARY, REWARD_AMOUNT), orml_tokens::Error::<Runtime>::BalanceTooLow);
		})
	}
}
