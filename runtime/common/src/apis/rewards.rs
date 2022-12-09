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

use codec::Codec;
use sp_api::decl_runtime_apis;
use sp_std::vec::Vec;

decl_runtime_apis! {
	/// Runtime API for the rewards pallet.
	pub trait RewardsApi<AccountId, Balance, DomainId, CurrencyId>
	where
		AccountId: Codec,
		Balance: Codec,
		DomainId: Codec,
		CurrencyId: Codec,
	{
		fn list_currencies(account_id: AccountId) -> Option<Vec<(DomainId, CurrencyId)>>;

		fn compute_reward(reward_currency_id: (DomainId, CurrencyId), account_id: AccountId) -> Option<Balance>;
	}
}
