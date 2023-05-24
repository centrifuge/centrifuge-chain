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
	BalanceStatus, Currency, ExistenceRequirement, LockIdentifier, LockableCurrency,
	ReservableCurrency, SignedImbalance, WithdrawReasons,
};

use super::*;

/// Represents the trait `Currency` effects that are called via
/// the pallet-restricted-tokens.
pub enum CurrencyEffects<AccountId, Balance> {
	/// A call to the `Currency::can_slash()`.
	///
	/// Interpretation of tuple `(AccountId, Balance, bool)`:
	/// * tuple.0 = `who`. The person who should be slashed.
	/// * tuple.1 = `value`. The to be slashed amount.
	/// * tuple.2 = `<T::NativeFungible as Currency>::can_slash()`. The result
	///   of the call to the not-filtered trait `Currency` implementation.
	CanSlash(AccountId, Balance, bool),

	/// A call to the `Currency::can_withdraw()`.
	///
	/// Interpretation of tuple `(AccountId, Balance, WitdrawReasons, Balance,
	/// DispatchResult)`:
	/// * tuple.0 = `who`. The person who's balance should be altered.
	/// * tuple.1 = `_amount`. The amount that should be withdrawn.
	/// * tuple.2 = `reasons`. The reasons for the withdraw.
	/// * tuple.3 = `new_balance`. The balance the account has after the
	///   withdraw.
	/// * tuple.4 = `<T::NativeFungible as Currency>::can_withdraw()`. The
	///   result of the call to the not-filtered trait `Currency`
	///   implementation.
	EnsureCanWithdraw(AccountId, Balance, WithdrawReasons, Balance, DispatchResult),

	/// A call to the `Currency::transfer()`.
	///
	/// Interpretation of tuple `(AccountId, AccountId, Balance,
	/// ExistenceRequirement)`:
	/// * tuple.0 = `sender`. The person who should be taken from.
	/// * tuple.1 = `receiver`. The person who should receive.
	/// * tuple.2 = `amount`. The amount that should be transfered.
	/// * tuple.3 = `existence`. The requirements the accounts must have after
	///   the transfer.
	Transfer(AccountId, AccountId, Balance, ExistenceRequirement),

	/// A call to the `Currency::withdraw()`.
	///
	/// Interpretation of tuple `(AccountId, Balance, WitdrawReasons,
	/// ExistenceRequirement)`:
	/// * tuple.0 = `who`. The person who's balance should be altered.
	/// * tuple.1 = `amount`. The amount that should be withdrawn.
	/// * tuple.2 = `reasons`. The reasons for the withdraw.
	/// * tuple.3 = `existence`. The requirements the accounts must have after
	///   the transfer.
	Withdraw(AccountId, Balance, WithdrawReasons, ExistenceRequirement),

	/// A call to the `Currency::deposit_into_existing()`.
	///
	/// Interpretation of tuple `(AccountId, Balance)`:
	/// * tuple.0 = `who`. The person who's balance should be deposit into.
	/// * tuple.1 = `amount`. The amount that should be deposited.
	DepositIntoExisting(AccountId, Balance),
}

impl<T: Config> Currency<T::AccountId> for Pallet<T> {
	type Balance = T::Balance;
	type NegativeImbalance = <T::NativeFungible as Currency<T::AccountId>>::NegativeImbalance;
	type PositiveImbalance = <T::NativeFungible as Currency<T::AccountId>>::PositiveImbalance;

	fn total_balance(who: &T::AccountId) -> Self::Balance {
		<T::NativeFungible as Currency<T::AccountId>>::total_balance(who)
	}

	fn can_slash(who: &T::AccountId, value: Self::Balance) -> bool {
		T::PreCurrency::check(CurrencyEffects::CanSlash(
			who.clone(),
			value,
			<T::NativeFungible as Currency<T::AccountId>>::can_slash(who, value),
		))
	}

	fn total_issuance() -> Self::Balance {
		<T::NativeFungible as Currency<T::AccountId>>::total_issuance()
	}

	fn minimum_balance() -> Self::Balance {
		<T::NativeFungible as Currency<T::AccountId>>::minimum_balance()
	}

	fn burn(amount: Self::Balance) -> Self::PositiveImbalance {
		<T::NativeFungible as Currency<T::AccountId>>::burn(amount)
	}

	fn issue(amount: Self::Balance) -> Self::NegativeImbalance {
		<T::NativeFungible as Currency<T::AccountId>>::issue(amount)
	}

	fn free_balance(who: &T::AccountId) -> Self::Balance {
		<T::NativeFungible as Currency<T::AccountId>>::free_balance(who)
	}

	fn ensure_can_withdraw(
		who: &T::AccountId,
		_amount: Self::Balance,
		reasons: WithdrawReasons,
		new_balance: Self::Balance,
	) -> DispatchResult {
		ensure!(
			T::PreCurrency::check(CurrencyEffects::EnsureCanWithdraw(
				who.clone(),
				_amount,
				reasons,
				new_balance,
				<T::NativeFungible as Currency<T::AccountId>>::ensure_can_withdraw(
					who,
					_amount,
					reasons,
					new_balance,
				)
			)),
			Error::<T>::PreConditionsNotMet
		);

		Ok(())
	}

	fn transfer(
		source: &T::AccountId,
		dest: &T::AccountId,
		value: Self::Balance,
		existence_requirement: ExistenceRequirement,
	) -> DispatchResult {
		ensure!(
			T::PreCurrency::check(CurrencyEffects::Transfer(
				source.clone(),
				dest.clone(),
				value,
				existence_requirement
			)),
			Error::<T>::PreConditionsNotMet
		);

		<T::NativeFungible as Currency<T::AccountId>>::transfer(
			source,
			dest,
			value,
			existence_requirement,
		)
	}

	fn slash(who: &T::AccountId, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
		<T::NativeFungible as Currency<T::AccountId>>::slash(who, value)
	}

	fn deposit_into_existing(
		who: &T::AccountId,
		value: Self::Balance,
	) -> Result<Self::PositiveImbalance, DispatchError> {
		ensure!(
			T::PreCurrency::check(CurrencyEffects::DepositIntoExisting(who.clone(), value,)),
			Error::<T>::PreConditionsNotMet
		);

		<T::NativeFungible as Currency<T::AccountId>>::deposit_into_existing(who, value)
	}

	fn deposit_creating(who: &T::AccountId, value: Self::Balance) -> Self::PositiveImbalance {
		<T::NativeFungible as Currency<T::AccountId>>::deposit_creating(who, value)
	}

	fn withdraw(
		who: &T::AccountId,
		value: Self::Balance,
		reasons: WithdrawReasons,
		liveness: ExistenceRequirement,
	) -> Result<Self::NegativeImbalance, DispatchError> {
		ensure!(
			T::PreCurrency::check(CurrencyEffects::Withdraw(
				who.clone(),
				value,
				reasons,
				liveness
			)),
			Error::<T>::PreConditionsNotMet
		);

		<T::NativeFungible as Currency<T::AccountId>>::withdraw(who, value, reasons, liveness)
	}

	fn make_free_balance_be(
		who: &T::AccountId,
		balance: Self::Balance,
	) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
		<T::NativeFungible as Currency<T::AccountId>>::make_free_balance_be(who, balance)
	}
}

/// Represents the trait `Currency` effects that are called via
/// the pallet-restricted-tokens.
pub enum ReservableCurrencyEffects<AccountId, Balance> {
	/// A call to the `ReservableCurrency::can_reserve()`.
	///
	/// Interpretation of tuple `(AccountId, Balance, bool)`:
	/// * tuple.0 = `who`. The person who's balance should be reserved.
	/// * tuple.1 = `amount`. The amount that should be reserved.
	/// * tuple.2 = `<T::NativeFungible as ReservableCurrency>::can_reserve()`.
	///   The result of the call to the not-filtered trait `ReservableCurrency`
	///   implementation.
	CanReserve(AccountId, Balance, bool),

	/// A call to the `ReservableCurrency::reserve()`.
	///
	/// Interpretation of tuple `(AccountId, Balance, bool)`:
	/// * tuple.0 = `who`. The person who's balance should be reserved.
	/// * tuple.1 = `amount`. The amount that should be reserved.
	Reserve(AccountId, Balance),

	/// A call to the `ReservableCurrency::repatriate_reserved()`.
	///
	/// Interpretation of tuple `(AccountId, Balance, bool)`:
	/// * tuple.0 = `slashed`. The account who's slashed.
	/// * tuple.1 = `beneficiary`. The account that benefits from the slash.
	/// * tuple.2 = `amount`. The amount that should be deducted.
	/// * tuple.3 = `status`. The status the funds will be placed into.
	RepatriateReserved(AccountId, AccountId, Balance, BalanceStatus),
}

impl<T: Config> ReservableCurrency<T::AccountId> for Pallet<T> {
	fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
		T::PreReservableCurrency::check(ReservableCurrencyEffects::CanReserve(
			who.clone(),
			value,
			<T::NativeFungible as ReservableCurrency<T::AccountId>>::can_reserve(who, value),
		))
	}

	fn slash_reserved(
		who: &T::AccountId,
		value: Self::Balance,
	) -> (Self::NegativeImbalance, Self::Balance) {
		<T::NativeFungible as ReservableCurrency<T::AccountId>>::slash_reserved(who, value)
	}

	fn reserved_balance(who: &T::AccountId) -> Self::Balance {
		<T::NativeFungible as ReservableCurrency<T::AccountId>>::reserved_balance(who)
	}

	fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
		ensure!(
			T::PreReservableCurrency::check(ReservableCurrencyEffects::Reserve(who.clone(), value)),
			Error::<T>::PreConditionsNotMet
		);
		<T::NativeFungible as ReservableCurrency<T::AccountId>>::reserve(who, value)
	}

	fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		<T::NativeFungible as ReservableCurrency<T::AccountId>>::unreserve(who, value)
	}

	fn repatriate_reserved(
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> Result<Self::Balance, DispatchError> {
		ensure!(
			T::PreReservableCurrency::check(ReservableCurrencyEffects::RepatriateReserved(
				slashed.clone(),
				beneficiary.clone(),
				value,
				status
			)),
			Error::<T>::PreConditionsNotMet
		);
		<T::NativeFungible as ReservableCurrency<T::AccountId>>::repatriate_reserved(
			slashed,
			beneficiary,
			value,
			status,
		)
	}
}

impl<T: Config> LockableCurrency<T::AccountId> for Pallet<T> {
	type MaxLocks = <T::NativeFungible as LockableCurrency<T::AccountId>>::MaxLocks;
	type Moment = <T::NativeFungible as LockableCurrency<T::AccountId>>::Moment;

	fn set_lock(
		id: LockIdentifier,
		who: &T::AccountId,
		amount: Self::Balance,
		reasons: WithdrawReasons,
	) {
		<T::NativeFungible as LockableCurrency<T::AccountId>>::set_lock(id, who, amount, reasons)
	}

	fn extend_lock(
		id: LockIdentifier,
		who: &T::AccountId,
		amount: Self::Balance,
		reasons: WithdrawReasons,
	) {
		<T::NativeFungible as LockableCurrency<T::AccountId>>::extend_lock(id, who, amount, reasons)
	}

	fn remove_lock(id: LockIdentifier, who: &T::AccountId) {
		<T::NativeFungible as LockableCurrency<T::AccountId>>::remove_lock(id, who)
	}
}
