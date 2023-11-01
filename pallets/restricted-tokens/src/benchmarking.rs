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
use cfg_primitives::{PoolId, TrancheId};
use cfg_traits::Permissions;
use cfg_types::{
	permissions::{PermissionScope, PoolRole, Role},
	tokens::CurrencyId,
};
use frame_benchmarking::{account, benchmarks, Zero};
use frame_support::traits::{
	fungibles,
	tokens::{Fortitude, Preservation},
	Get,
};
use frame_system::RawOrigin;
use orml_traits::GetByKey;
use sp_runtime::traits::StaticLookup;
use sp_std::default::Default;

use super::*;

const CURRENCY: u128 = 1_000_000_000_000_000_000u128;

fn make_free_balance<T>(
	currency_id: <T as Config>::CurrencyId,
	account: &T::AccountId,
	balance: <T as Config>::Balance,
) where
	T: Config
		+ pallet_balances::Config<Balance = <T as Config>::Balance, HoldIdentifier = ()>
		+ orml_tokens::Config<
			Balance = <T as Config>::Balance,
			CurrencyId = <T as Config>::CurrencyId,
		>,
{
	if T::NativeToken::get() == currency_id {
		assert_eq!(
			<pallet_balances::Pallet<T> as fungible::Mutate<T::AccountId>>::mint_into(
				account, balance
			)
			.expect("should not fail to set native tokens"),
			balance
		);
	} else {
		assert_eq!(
			<orml_tokens::Pallet<T> as fungibles::Mutate<T::AccountId>>::mint_into(
				currency_id,
				account,
				balance,
			)
			.expect("should not fail to set tokens"),
			balance
		);
	}
}

fn reserve_balance<T>(
	currency_id: <T as Config>::CurrencyId,
	account: &T::AccountId,
	balance: <T as Config>::Balance,
) where
	T: Config
		+ pallet_balances::Config<Balance = <T as Config>::Balance, HoldIdentifier = ()>
		+ orml_tokens::Config<
			Balance = <T as Config>::Balance,
			CurrencyId = <T as Config>::CurrencyId,
		>,
{
	if T::NativeToken::get() == currency_id {
		assert!(
			frame_system::Pallet::<T>::providers(account) > 0,
			"Providers should not be zero"
		);
		<pallet_balances::Pallet<T> as fungible::MutateHold<T::AccountId>>::hold(
			&Default::default(),
			account,
			balance,
		)
		.expect("should not fail to hold existing native tokens");
	} else {
		<orml_tokens::Pallet<T> as fungibles::MutateHold<T::AccountId>>::hold(
			currency_id,
			&Default::default(),
			account,
			balance,
		)
		.expect("should not fail to hold existing foreign tokens");
	}
}

fn whitelist_acc<T: frame_system::Config>(acc: &T::AccountId) {
	frame_benchmarking::benchmarking::add_to_whitelist(
		frame_system::Account::<T>::hashed_key_for(acc).into(),
	);
}

fn get_account<T>(name: &'static str, whitelist: bool) -> T::AccountId
where
	T: frame_system::Config,
{
	let acc = account::<T::AccountId>(name, 0, 0);
	if whitelist {
		whitelist_acc::<T>(&acc);
	}
	acc
}

fn get_account_maybe_permission<T>(name: &'static str, currency: T::CurrencyId) -> T::AccountId
where
	T: Config
		+ pallet_permissions::Config<Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>,
	T::CurrencyId: Into<CurrencyId>,
{
	let acc = get_account::<T>(name, false);
	if let CurrencyId::Tranche(pool_id, tranche_id) = currency.into() {
		permission_for_tranche::<T>(acc.clone(), pool_id, tranche_id);
	}

	acc
}

fn permission_for_tranche<T>(acc: T::AccountId, pool_id: PoolId, tranche_id: TrancheId)
where
	T: frame_system::Config
		+ pallet_permissions::Config<Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>,
{
	<pallet_permissions::Pallet<T> as Permissions<T::AccountId>>::add(
		PermissionScope::Pool(pool_id),
		acc,
		Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, u64::MAX)),
	)
	.expect("Whitelisting works. qed.");
}

fn set_up_account<T>(
	name: &'static str,
	currency: <T as Config>::CurrencyId,
	amount: <T as Config>::Balance,
	reserved: Option<<T as Config>::Balance>,
) -> T::AccountId
where
	T: Config
		+ pallet_balances::Config<Balance = <T as Config>::Balance, HoldIdentifier = ()>
		+ orml_tokens::Config<
			Balance = <T as Config>::Balance,
			CurrencyId = <T as Config>::CurrencyId,
		> + pallet_permissions::Config<Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>,
	<T as Config>::CurrencyId: Into<CurrencyId>,
{
	let acc = get_account::<T>(name, true);
	make_free_balance::<T>(currency, &acc, amount);

	if let Some(reserve) = reserved {
		reserve_balance::<T>(currency, &acc, reserve);
	}

	if let CurrencyId::Tranche(pool_id, tranche_id) = currency.into() {
		permission_for_tranche::<T>(acc.clone(), pool_id, tranche_id);
	}

	acc
}

fn as_balance<T>(amount: u32) -> T::Balance
where
	T: Config,
	<T as Config>::Balance: From<u128>,
{
	Into::<T::Balance>::into(Into::<u128>::into(amount) * CURRENCY)
}

fn get_non_native_currency<T>() -> T::CurrencyId
where
	T: Config,
	T::CurrencyId: From<CurrencyId>,
{
	CurrencyId::ForeignAsset(1).into()
}

benchmarks! {
	where_clause {
		where
		T: Config
			+ pallet_balances::Config<Balance = <T as Config>::Balance, HoldIdentifier = ()>
			+ orml_tokens::Config<Balance = <T as Config>::Balance, CurrencyId = <T as Config>::CurrencyId>
			+ pallet_permissions::Config<Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>,
		<T as Config>::Balance: From<u128> + Zero,
		<T as Config>::CurrencyId: From<CurrencyId> + Into<CurrencyId>,
	}

	// We transfer into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	transfer_native {
		let amount = as_balance::<T>(300);
		let currency: <T as Config>::CurrencyId = CurrencyId::Native.into();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account::<T>("receiver", false);
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:transfer(RawOrigin::Signed(send.clone()), recv_loopup, currency, amount)
	verify {
		assert!(pallet_balances::Pallet::<T>::free_balance(&recv) == amount);
		assert!(pallet_balances::Pallet::<T>::free_balance(&send) == Zero::zero());
	}

	// We benchmark into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	transfer_other {
		let amount = as_balance::<T>(300);
		let currency = get_non_native_currency::<T>();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:transfer(RawOrigin::Signed(send.clone()), recv_loopup, currency.clone(), amount)
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, Preservation::Protect, Fortitude::Polite) == amount);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &send, Preservation::Protect, Fortitude::Polite) == Zero::zero());
	}

	transfer_keep_alive_native {
		let amount = as_balance::<T>(300);
		let min_deposit = <T as pallet_balances::Config>::ExistentialDeposit::get();
		let send_amount = amount - min_deposit;
		let currency: <T as Config>::CurrencyId = CurrencyId::Native.into();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account::<T>("receiver", false);
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:transfer_keep_alive(RawOrigin::Signed(send.clone()), recv_loopup, currency, send_amount)
	verify {
		assert!(pallet_balances::Pallet::<T>::free_balance(&recv) == send_amount);
		assert!(pallet_balances::Pallet::<T>::free_balance(&send) == amount - send_amount);
	}

	// We benchmark into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	transfer_keep_alive_other {
		let amount = as_balance::<T>(300);
		let currency = get_non_native_currency::<T>();
		let min_deposit = <T as orml_tokens::Config>::ExistentialDeposits::get(&currency);
		let send_amount = amount - min_deposit;
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:transfer_keep_alive(RawOrigin::Signed(send.clone()), recv_loopup, currency, send_amount)
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, Preservation::Protect, Fortitude::Polite)  == send_amount);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &send, Preservation::Protect, Fortitude::Polite)  == amount - send_amount);
	}

	// We transfer into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	// We let the other die to have clean-up logic in weight
	transfer_all_native {
		let amount = as_balance::<T>(300);
		let currency: <T as Config>::CurrencyId = CurrencyId::Native.into();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account::<T>("receiver", false);
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:transfer_all(RawOrigin::Signed(send.clone()), recv_loopup, currency)
	verify {
		assert!(pallet_balances::Pallet::<T>::free_balance(&recv) == amount);
		assert!(pallet_balances::Pallet::<T>::free_balance(&send) == Zero::zero());
	}

	// We benchmark into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	// We let the other die to have clean-up logic in weight
	transfer_all_other {
		let amount = as_balance::<T>(300);
		let currency: <T as Config>::CurrencyId = get_non_native_currency::<T>();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:transfer_all(RawOrigin::Signed(send.clone()), recv_loopup, currency.clone())
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, Preservation::Protect, Fortitude::Polite) == amount);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &send, Preservation::Protect, Fortitude::Polite) == Zero::zero());
	}

	// We transfer into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	// We let the other die to have clean-up logic in weight
	force_transfer_native {
		let amount = as_balance::<T>(300);
		let currency: <T as Config>::CurrencyId = CurrencyId::Native.into();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account::<T>("receiver", false);
		let send_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(send.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:force_transfer(RawOrigin::Root, send_loopup, recv_loopup, currency, amount)
	verify {
		assert!(pallet_balances::Pallet::<T>::free_balance(&recv) == amount);
		assert!(pallet_balances::Pallet::<T>::free_balance(&send) == Zero::zero());
	}

	// We benchmark into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	// We let the other die to have clean-up logic in weight
	force_transfer_other {
		let amount = as_balance::<T>(300);
		let currency: <T as Config>::CurrencyId = get_non_native_currency::<T>();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let send_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(send.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:force_transfer(RawOrigin::Root, send_loopup, recv_loopup, currency.clone(), amount)
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, Preservation::Protect, Fortitude::Polite) == amount);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &send, Preservation::Protect, Fortitude::Polite) == Zero::zero());
	}

	// We fund the account beforehand to get worst-case scenario (release
	// held funds and burn all tokens).
	// It might be beneficially to have a separation of cases in the future.
	// We let the other die to have clean-up logic in weight
	set_balance_native {
		let free = as_balance::<T>(300);
		let reserved = as_balance::<T>(200);
		let currency: <T as Config>::CurrencyId = CurrencyId::Native.into();
		let recv = get_account::<T>("receiver", false);
		let recv_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());

		make_free_balance::<T>(currency, &recv, free + free);
		reserve_balance::<T>(currency, &recv, reserved + reserved);
	}:set_balance(RawOrigin::Root, recv_lookup, currency, free, reserved)
	verify {
		assert!(<pallet_balances::Pallet<T> as fungible::InspectHold<T::AccountId>>::total_balance_on_hold(&recv) == reserved);
		assert!(<pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::reducible_balance(&recv, Preservation::Protect, Fortitude::Polite) == free - <pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::minimum_balance());
		assert!(<pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::balance(&recv) == (free));
	}

	// We fund the account beforehand to get worst-case scenario (release
	// held funds and burn all tokens).
	// It might be beneficially to have a separation of cases in the future.
	// We let the other die to have clean-up logic in weight
	set_balance_other {
		let free = as_balance::<T>(300);
		let reserved = as_balance::<T>(200);
		let currency: <T as Config>::CurrencyId = get_non_native_currency::<T>();
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());

		make_free_balance::<T>(currency, &recv, free + free);
		reserve_balance::<T>(currency, &recv, reserved + reserved);
	}:set_balance(RawOrigin::Root, recv_loopup, currency.clone(), free, reserved)
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::InspectHold<T::AccountId>>::total_balance_on_hold(currency, &recv) == reserved);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, Preservation::Protect, Fortitude::Polite) == free - <orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::minimum_balance(currency));
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::balance(currency, &recv) == (free));
	}
}
