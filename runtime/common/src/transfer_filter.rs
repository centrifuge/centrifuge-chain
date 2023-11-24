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

use cfg_primitives::{AccountId, Balance};
use cfg_traits::PreConditions;
use cfg_types::tokens::CurrencyId;
use pallet_restricted_xtokens::TransferEffects;
use sp_runtime::DispatchResult;

pub struct PreXcmTransfer<T>(sp_std::marker::PhantomData<T>);

impl<T> PreConditions<TransferEffects<AccountId, CurrencyId, Balance>> for PreXcmTransfer<T> {
	type Result = DispatchResult;

	fn check(t: TransferEffects<AccountId, CurrencyId, Balance>) -> Self::Result {
		todo!()
	}
}
