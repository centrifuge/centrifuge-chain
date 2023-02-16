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
use codec::{Decode, Encode};
use frame_support::traits::fungibles::{Mutate, Transfer};
use sp_runtime::{traits::Get, DispatchResult};
use sp_std::marker::PhantomData;

/// Enables rewarding out of thin air, e.g. via minting.
// TODO: Add unit tests
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
		Currency::mint_into(currency_id, beneficiary, amount)
	}
}

/// Enables rewarding from an account address, e.g. the Treasury.
// TODO: Add unit tests
pub struct TransferReward<AccountId, Balance, CurrencyId, Currency, SourceAddress>(
	PhantomData<(AccountId, Balance, CurrencyId, Currency, SourceAddress)>,
);

impl<AccountId, Balance, CurrencyId, Currency, SourceAddress> RewardIssuance
	for TransferReward<AccountId, Balance, CurrencyId, Currency, SourceAddress>
where
	AccountId: Encode + Decode,
	Currency: Transfer<AccountId, AssetId = CurrencyId, Balance = Balance>,
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
			true,
		)
		.map(|_| ())
	}
}
