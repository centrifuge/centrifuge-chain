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
use crate::impl_fungible::{
	FungibleMutateEffects, FungibleMutateHoldEffects, FungibleTransferEffects,
};
use common_traits::PreConditions;
use frame_support::traits::{
	fungible,
	fungibles::{Inspect, InspectHold, Mutate, MutateHold, Transfer},
	tokens::{DepositConsequence, WithdrawConsequence},
};

impl<T: Config> Inspect<T::AccountId> for Pallet<T> {
	type AssetId = T::CurrencyId;
	type Balance = T::Balance;

	fn total_issuance(asset: Self::AssetId) -> Self::Balance {
		if asset == T::NativeToken::get() {
			<T::NativeFungible as fungible::Inspect<T::AccountId>>::total_issuance()
		} else {
			<T::Fungibles as Inspect<T::AccountId>>::total_issuance(asset)
		}
	}

	fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
		if asset == T::NativeToken::get() {
			<T::NativeFungible as fungible::Inspect<T::AccountId>>::minimum_balance()
		} else {
			<T::Fungibles as Inspect<T::AccountId>>::minimum_balance(asset)
		}
	}

	fn balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
		// TODO: Actually, a filter would be nice here.. but how?

		if asset == T::NativeToken::get() {
			<T::NativeFungible as fungible::Inspect<T::AccountId>>::balance(who)
		} else {
			<T::Fungibles as Inspect<T::AccountId>>::balance(asset, who)
		}
	}

	fn reducible_balance(
		asset: Self::AssetId,
		who: &T::AccountId,
		keep_alive: bool,
	) -> Self::Balance {
		// TODO: Actually, a filter would be nice here.. but how?

		if asset == T::NativeToken::get() {
			<T::NativeFungible as fungible::Inspect<T::AccountId>>::reducible_balance(
				who, keep_alive,
			)
		} else {
			<T::Fungibles as Inspect<T::AccountId>>::reducible_balance(asset, who, keep_alive)
		}
	}

	fn can_deposit(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DepositConsequence {
		// TODO: Actually, a filter would be nice here.. but how?

		if asset == T::NativeToken::get() {
			<T::NativeFungible as fungible::Inspect<T::AccountId>>::can_deposit(who, amount)
		} else {
			<T::Fungibles as Inspect<T::AccountId>>::can_deposit(asset, who, amount)
		}
	}

	fn can_withdraw(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		// TODO: Actually, a filter would be nice here.. but how?

		if asset == T::NativeToken::get() {
			<T::NativeFungible as fungible::Inspect<T::AccountId>>::can_withdraw(who, amount)
		} else {
			<T::Fungibles as Inspect<T::AccountId>>::can_withdraw(asset, who, amount)
		}
	}
}

impl<T: Config> InspectHold<T::AccountId> for Pallet<T> {
	fn balance_on_hold(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
		if asset == T::NativeToken::get() {
			<T::NativeFungible as fungible::InspectHold<T::AccountId>>::balance_on_hold(who)
		} else {
			<T::Fungibles as InspectHold<T::AccountId>>::balance_on_hold(asset, who)
		}
	}

	fn can_hold(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> bool {
		// TODO: Actually, a filter would be nice here.. but how?

		if asset == T::NativeToken::get() {
			<T::NativeFungible as fungible::InspectHold<T::AccountId>>::can_hold(who, amount)
		} else {
			<T::Fungibles as InspectHold<T::AccountId>>::can_hold(asset, who, amount)
		}
	}
}

pub enum FungiblesMutateEffects<AssetId, AccountId, Balance> {
	MintInto(AssetId, AccountId, Balance),
	BurnFrom(AssetId, AccountId, Balance),
}

// TODO: Decide wether to manually implement `Mutate::slash()` and `Mutate::teleport`
impl<T: Config> Mutate<T::AccountId> for Pallet<T> {
	fn mint_into(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if asset == T::NativeToken::get() {
			ensure!(
				T::PreFungibleMutate::check(FungibleMutateEffects::MintInto(who.clone(), amount)),
				Error::<T>::PreConditionsNotMet
			);

			<T::NativeFungible as fungible::Mutate<T::AccountId>>::mint_into(who, amount)
		} else {
			ensure!(
				T::PreFungiblesMutate::check(FungiblesMutateEffects::MintInto(
					asset,
					who.clone(),
					amount
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::Fungibles as Mutate<T::AccountId>>::mint_into(asset, who, amount)
		}
	}

	fn burn_from(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> Result<Self::Balance, DispatchError> {
		if asset == T::NativeToken::get() {
			ensure!(
				T::PreFungibleMutate::check(FungibleMutateEffects::BurnFrom(who.clone(), amount)),
				Error::<T>::PreConditionsNotMet
			);

			<T::NativeFungible as fungible::Mutate<T::AccountId>>::burn_from(who, amount)
		} else {
			ensure!(
				T::PreFungiblesMutate::check(FungiblesMutateEffects::BurnFrom(
					asset,
					who.clone(),
					amount
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::Fungibles as Mutate<T::AccountId>>::burn_from(asset, who, amount)
		}
	}
}

pub enum FungiblesMutateHoldEffects<AssetId, AccountId, Balance> {
	Hold(AssetId, AccountId, Balance),
	Release(AssetId, AccountId, Balance, bool),
	TransferHeld(AssetId, AccountId, AccountId, Balance, bool, bool),
}

impl<T: Config> MutateHold<T::AccountId> for Pallet<T> {
	fn hold(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if asset == T::NativeToken::get() {
			ensure!(
				T::PreFungibleMutateHold::check(FungibleMutateHoldEffects::Hold(
					who.clone(),
					amount
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::NativeFungible as fungible::MutateHold<T::AccountId>>::hold(who, amount)
		} else {
			ensure!(
				T::PreFungiblesMutateHold::check(FungiblesMutateHoldEffects::Hold(
					asset,
					who.clone(),
					amount
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::Fungibles as MutateHold<T::AccountId>>::hold(asset, who, amount)
		}
	}

	fn release(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
		best_effort: bool,
	) -> Result<Self::Balance, DispatchError> {
		if asset == T::NativeToken::get() {
			ensure!(
				T::PreFungibleMutateHold::check(FungibleMutateHoldEffects::Release(
					who.clone(),
					amount,
					best_effort
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::NativeFungible as fungible::MutateHold<T::AccountId>>::release(
				who,
				amount,
				best_effort,
			)
		} else {
			ensure!(
				T::PreFungiblesMutateHold::check(FungiblesMutateHoldEffects::Release(
					asset,
					who.clone(),
					amount,
					best_effort
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::Fungibles as MutateHold<T::AccountId>>::release(asset, who, amount, best_effort)
		}
	}

	fn transfer_held(
		asset: Self::AssetId,
		source: &T::AccountId,
		dest: &T::AccountId,
		amount: Self::Balance,
		best_effort: bool,
		on_hold: bool,
	) -> Result<Self::Balance, DispatchError> {
		if asset == T::NativeToken::get() {
			ensure!(
				T::PreFungibleMutateHold::check(FungibleMutateHoldEffects::TransferHeld(
					source.clone(),
					dest.clone(),
					amount,
					best_effort,
					on_hold
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::NativeFungible as fungible::MutateHold<T::AccountId>>::transfer_held(
				source,
				dest,
				amount,
				best_effort,
				on_hold,
			)
		} else {
			ensure!(
				T::PreFungiblesMutateHold::check(FungiblesMutateHoldEffects::TransferHeld(
					asset,
					source.clone(),
					dest.clone(),
					amount,
					best_effort,
					on_hold
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::Fungibles as MutateHold<T::AccountId>>::transfer_held(
				asset,
				source,
				dest,
				amount,
				best_effort,
				on_hold,
			)
		}
	}
}

pub enum FungiblesTransferEffects<AssetId, AccountId, Balance> {
	Transfer(AssetId, AccountId, AccountId, Balance, bool),
}

impl<T: Config> Transfer<T::AccountId> for Pallet<T> {
	fn transfer(
		asset: Self::AssetId,
		source: &T::AccountId,
		dest: &T::AccountId,
		amount: Self::Balance,
		keep_alive: bool,
	) -> Result<Self::Balance, DispatchError> {
		if asset == T::NativeToken::get() {
			ensure!(
				T::PreFungibleTransfer::check(FungibleTransferEffects::Transfer(
					source.clone(),
					dest.clone(),
					amount,
					keep_alive
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::NativeFungible as fungible::Transfer<T::AccountId>>::transfer(
				source, dest, amount, keep_alive,
			)
		} else {
			ensure!(
				T::PreFungiblesTransfer::check(FungiblesTransferEffects::Transfer(
					asset,
					source.clone(),
					dest.clone(),
					amount,
					keep_alive
				)),
				Error::<T>::PreConditionsNotMet
			);

			<T::Fungibles as Transfer<T::AccountId>>::transfer(
				asset, source, dest, amount, keep_alive,
			)
		}
	}
}
