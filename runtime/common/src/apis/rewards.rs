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

use parity_scale_codec::{Codec, Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_api::decl_runtime_apis;
use sp_core::RuntimeDebug;
use sp_std::vec::Vec;

#[derive(Encode, Decode, Clone, Copy, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub enum RewardDomain {
	Block,
	Liquidity,
}

decl_runtime_apis! {
	/// Runtime API for the rewards pallet.
	pub trait RewardsApi<AccountId, Balance, CurrencyId>
	where
		AccountId: Codec,
		Balance: Codec,
		CurrencyId: Codec,
	{
		fn list_currencies(domain: RewardDomain, account_id: AccountId) -> Vec<CurrencyId>;

		fn compute_reward(domain: RewardDomain, currency_id: CurrencyId, account_id: AccountId) -> Option<Balance>;
	}
}
