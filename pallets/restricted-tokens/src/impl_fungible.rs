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

use super::*;
use common_traits::PreConditions;
use frame_support::traits::{
	fungible::{Inspect, InspectHold, Mutate, MutateHold, Transfer},
	tokens::{DepositConsequence, WithdrawConsequence},
};

impl<T: Config> Inspect<T::AccountId> for Pallet<T> {
	type Balance = T::Balance;

	fn total_issuance() -> Self::Balance {
		<T::NativeFungible as Inspect<T::AccountId>>::total_issuance()
	}

	fn minimum_balance() -> Self::Balance {
		<T::NativeFungible as Inspect<T::AccountId>>::minimum_balance()
	}

	fn balance(who: &T::AccountId) -> Self::Balance {
		// TODO: Actually, a filter would be nice here.. but how?
		<T::NativeFungible as Inspect<T::AccountId>>::balance(who)
	}

	fn reducible_balance(who: &T::AccountId, keep_alive: bool) -> Self::Balance {
		// TODO: Actually, a filter would be nice here.. but how?
		<T::NativeFungible as Inspect<T::AccountId>>::reducible_balance(who, keep_alive)
	}

	fn can_deposit(who: &T::AccountId, amount: Self::Balance) -> DepositConsequence {
		// TODO: Actually, a filter would be nice here.. but how?
		<T::NativeFungible as Inspect<T::AccountId>>::can_deposit(who, amount)
	}

	fn can_withdraw(
		who: &T::AccountId,
		amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		// TODO: Actually, a filter would be nice here.. but how?
		<T::NativeFungible as Inspect<T::AccountId>>::can_withdraw(who, amount)
	}
}

impl<T: Config> InspectHold<T::AccountId> for Pallet<T> {
	fn balance_on_hold(who: &T::AccountId) -> Self::Balance {
		// TODO: Actually, a filter would be nice here.. but how?
		<T::NativeFungible as InspectHold<T::AccountId>>::balance_on_hold(who)
	}

	fn can_hold(who: &T::AccountId, amount: Self::Balance) -> bool {
		// TODO: Actually, a filter would be nice here.. but how?
		<T::NativeFungible as InspectHold<T::AccountId>>::can_hold(who, amount)
	}
}

pub enum FungibleMutateEffects<AccountId, Balance> {
	MintInto(AccountId, Balance),
	BurnFrom(AccountId, Balance),
}

impl<T: Config> Mutate<T::AccountId> for Pallet<T> {
	fn mint_into(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		ensure!(
			T::PreFungibleMutate::check(FungibleMutateEffects::MintInto(who.clone(), amount)),
			Error::<T>::PreConditionsNotMet
		);

		<T::NativeFungible as Mutate<T::AccountId>>::mint_into(who, amount)
	}

	fn burn_from(
		who: &T::AccountId,
		amount: Self::Balance,
	) -> Result<Self::Balance, DispatchError> {
		ensure!(
			T::PreFungibleMutate::check(FungibleMutateEffects::BurnFrom(who.clone(), amount)),
			Error::<T>::PreConditionsNotMet
		);

		<T::NativeFungible as Mutate<T::AccountId>>::burn_from(who, amount)
	}
}

pub enum FungibleMutateHoldEffects<AccountId, Balance> {
	Hold(AccountId, Balance),
	Release(AccountId, Balance, bool),
	TransferHeld(AccountId, AccountId, Balance, bool, bool),
}

impl<T: Config> MutateHold<T::AccountId> for Pallet<T> {
	fn hold(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		ensure!(
			T::PreFungibleMutateHold::check(FungibleMutateHoldEffects::Hold(who.clone(), amount)),
			Error::<T>::PreConditionsNotMet
		);

		<T::NativeFungible as MutateHold<T::AccountId>>::hold(who, amount)
	}

	fn release(
		who: &T::AccountId,
		amount: Self::Balance,
		best_effort: bool,
	) -> Result<Self::Balance, DispatchError> {
		ensure!(
			T::PreFungibleMutateHold::check(FungibleMutateHoldEffects::Release(
				who.clone(),
				amount,
				best_effort
			)),
			Error::<T>::PreConditionsNotMet
		);

		<T::NativeFungible as MutateHold<T::AccountId>>::release(who, amount, best_effort)
	}

	fn transfer_held(
		source: &T::AccountId,
		dest: &T::AccountId,
		amount: Self::Balance,
		best_effort: bool,
		on_held: bool,
	) -> Result<Self::Balance, DispatchError> {
		ensure!(
			T::PreFungibleMutateHold::check(FungibleMutateHoldEffects::TransferHeld(
				source.clone(),
				dest.clone(),
				amount,
				best_effort,
				on_held
			)),
			Error::<T>::PreConditionsNotMet
		);

		<T::NativeFungible as MutateHold<T::AccountId>>::transfer_held(
			source,
			dest,
			amount,
			best_effort,
			on_held,
		)
	}
}

pub enum FungibleTransferEffects<AccountId, Balance> {
	Transfer(AccountId, AccountId, Balance, bool),
}

impl<T: Config> Transfer<T::AccountId> for Pallet<T> {
	fn transfer(
		source: &T::AccountId,
		dest: &T::AccountId,
		amount: Self::Balance,
		keep_alive: bool,
	) -> Result<Self::Balance, DispatchError> {
		ensure!(
			T::PreFungibleTransfer::check(FungibleTransferEffects::Transfer(
				source.clone(),
				dest.clone(),
				amount,
				keep_alive
			)),
			Error::<T>::PreConditionsNotMet
		);

		<T::NativeFungible as Transfer<T::AccountId>>::transfer(source, dest, amount, keep_alive)
	}
}
