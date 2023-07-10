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

use frame_support::traits::fungible::Mutate;
use pallet_evm::{AddressMapping, OnCreate};
use sp_core::{crypto::AccountId32, Get, H160};

use crate::account_conversion::AccountConverter;

pub struct ContractCreation<R>(core::marker::PhantomData<R>);

impl<R> OnCreate<R> for ContractCreation<R>
where
	R: pallet_balances::Config + pallet_evm_chain_id::Config,
	R::AccountId: From<AccountId32>,
{
	fn on_create(_owner: H160, contract: H160) {
		let contract_account = AccountConverter::<R>::into_account_id(contract);

		let existential_deposit = <R as pallet_balances::Config>::ExistentialDeposit::get();

		// TODO(cdamian): Given that this trait does not return anything, should we
		// panic here?
		let _ =
			pallet_balances::Pallet::<R>::mint_into(&contract_account.into(), existential_deposit);
	}
}
