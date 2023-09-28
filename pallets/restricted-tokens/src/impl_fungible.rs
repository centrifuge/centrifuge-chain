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

use cfg_traits::PreConditions;
use frame_support::traits::{
	fungible::{Inspect, InspectHold, Mutate, MutateHold},
	tokens::{DepositConsequence, WithdrawConsequence},
};

use super::*;

/// Represents the trait `fungible::Inspect` effects that are called via
/// the pallet-restricted-tokens.
pub enum FungibleInspectEffects<AccountId, Balance> {
	/// A call to the `Inspect::reducible_balance()`.
	///
	/// Interpretation of tuple `(AccountId, bool, Balance)`:
	/// * tuple.0 = `who`. The person who's balance should be checked.
	/// * tuple.1 = `keep_alive`. The liveness bool.
	/// * tuple.2 = `<T::NativeFungible as
	///   Inspect<T::AccountId>>::reducible_balance()`. The result of the call
	///   to the not-filtered trait `fungible::Inspect` implementation.
	ReducibleBalance(AccountId, bool, Balance),
}

pub struct FungibleInspectPassthrough;
impl<AccountId, Balance> PreConditions<FungibleInspectEffects<AccountId, Balance>>
	for FungibleInspectPassthrough
{
	type Result = Balance;

	fn check(t: FungibleInspectEffects<AccountId, Balance>) -> Self::Result {
		match t {
			FungibleInspectEffects::ReducibleBalance(_, _, amount) => amount,
		}
	}
}

impl<T: Config> Inspect<T::AccountId> for Pallet<T> {
	type Balance = T::Balance;

	fn total_issuance() -> Self::Balance {
		<T::NativeFungible as Inspect<T::AccountId>>::total_issuance()
	}

	fn minimum_balance() -> Self::Balance {
		<T::NativeFungible as Inspect<T::AccountId>>::minimum_balance()
	}

	fn balance(who: &T::AccountId) -> Self::Balance {
		<T::NativeFungible as Inspect<T::AccountId>>::balance(who)
	}

	fn reducible_balance(who: &T::AccountId, keep_alive: bool) -> Self::Balance {
		T::PreFungibleInspect::check(FungibleInspectEffects::ReducibleBalance(
			who.clone(),
			keep_alive,
			<T::NativeFungible as Inspect<T::AccountId>>::reducible_balance(who, keep_alive),
		))
	}

	fn can_deposit(who: &T::AccountId, amount: Self::Balance, mint: bool) -> DepositConsequence {
		<T::NativeFungible as Inspect<T::AccountId>>::can_deposit(who, amount, mint)
	}

	fn can_withdraw(
		who: &T::AccountId,
		amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		<T::NativeFungible as Inspect<T::AccountId>>::can_withdraw(who, amount)
	}
}
/// Represents the trait `fungible::InspectHold` effects that are called via
/// the pallet-restricted-tokens.
pub enum FungibleInspectHoldEffects<AccountId, Balance> {
	/// A call to the `InspectHold::can_hold()`.
	///
	/// Interpretation of tuple `(AccountId, Balance, bool)`:
	/// * tuple.0 = `who`. The person who's balance should be reserved.
	/// * tuple.1 = `amount`. The amount that should be reserved.
	/// * tuple.2 = `<T::NativeFungible as
	///   InspectHold<T::AccountId>>::can_hold()`. The result of the call to the
	///   not-filtered trait `fungible::InspectHold` implementation.
	CanHold(AccountId, Balance, bool),
}

impl<T: Config> InspectHold<T::AccountId> for Pallet<T> {
	fn balance_on_hold(who: &T::AccountId) -> Self::Balance {
		<T::NativeFungible as InspectHold<T::AccountId>>::balance_on_hold(who)
	}

	fn can_hold(who: &T::AccountId, amount: Self::Balance) -> bool {
		T::PreFungibleInspectHold::check(FungibleInspectHoldEffects::CanHold(
			who.clone(),
			amount,
			<T::NativeFungible as InspectHold<T::AccountId>>::can_hold(who, amount),
		))
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

/// Represents the trait `fungible::MutateHold` effects that are called via
/// the pallet-restricted-tokens.
pub enum FungibleMutateHoldEffects<AccountId, Balance> {
	/// A call to the `MutateHold::hold()`.
	///
	/// Interpretation of tuple `(AccountId, Balance)`:
	/// * tuple.0 = `who`. The person who's balance should be altered.
	/// * tuple.1 = `amount`. The amount that should be hold.
	Hold(AccountId, Balance),

	/// A call to the `MutateHold::release()`.
	///
	/// Interpretation of tuple `(AccountId, Balance)`:
	/// * tuple.0 = `who`. The person who's balance should be altered.
	/// * tuple.1 = `amount`. The amount that should be released.
	Release(AccountId, Balance, bool),

	/// A call to the `MutateHold::transfer_held()`.
	///
	/// Interpretation of tuple `(AccountId, AccountId, Balance, bool, bool)`:
	/// * tuple.0 = `send`. The sender of the tokens.
	/// * tuple.1 = `recv`. The receiver of the tokens.
	/// * tuple.2 = `amount`. The amount that should be transferred.
	/// * tuple.3 = `on_hold`. Indicating if on_hold transfers should still be
	///   on_hold at receiver.
	/// * tuple.4 = `best_effort`. Indicating if the transfer should be done on
	///   a best effort base.
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

/// Represents the trait `fungible::Transfer` effects that are called via
/// the pallet-restricted-tokens.
pub enum FungibleTransferEffects<AccountId, Balance> {
	/// A call to the `Transfer::transfer()`.
	///
	/// Interpretation of tuple `(AccountId, AccountId, Balance, bool)`:
	/// * tuple.0 = `send`. The sender of the tokens.
	/// * tuple.1 = `recv`. The receiver of the tokens.
	/// * tuple.2 = `amount`. The amount that should be transferred.
	/// * tuple.3 = `keep_alive`. The lifeness requirements.
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
