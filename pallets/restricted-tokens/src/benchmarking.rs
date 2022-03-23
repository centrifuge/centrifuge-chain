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
use common_traits::Permissions;
use common_types::{CurrencyId, PoolRole};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, Zero};
use frame_support::traits::{fungibles, Get};
use frame_system::RawOrigin;
use orml_traits::GetByKey;
use runtime_common::{PoolId, TrancheId};
use sp_runtime::traits::StaticLookup;

const CURRENCY: u128 = 1_000_000_000_000_000_000u128;

fn make_free_balance<T>(
	currency_id: <T as Config>::CurrencyId,
	account: &T::AccountId,
	balance: <T as Config>::Balance,
) where
	T: Config
		+ pallet_balances::Config<Balance = <T as Config>::Balance>
		+ orml_tokens::Config<
			Balance = <T as Config>::Balance,
			CurrencyId = <T as Config>::CurrencyId,
		>,
{
	if T::NativeToken::get() == currency_id {
		<pallet_balances::Pallet<T> as fungible::Mutate<T::AccountId>>::mint_into(account, balance)
			.expect("should not fail to set tokens");
	} else {
		<orml_tokens::Pallet<T> as fungibles::Mutate<T::AccountId>>::mint_into(
			currency_id,
			account,
			balance,
		)
		.expect("should not fail to set tokens");
	}
}

fn reserve_balance<T>(
	currency_id: <T as Config>::CurrencyId,
	account: &T::AccountId,
	balance: <T as Config>::Balance,
) where
	T: Config
		+ pallet_balances::Config<Balance = <T as Config>::Balance>
		+ orml_tokens::Config<
			Balance = <T as Config>::Balance,
			CurrencyId = <T as Config>::CurrencyId,
		>,
{
	if T::NativeToken::get() == currency_id {
		<pallet_balances::Pallet<T> as fungible::MutateHold<T::AccountId>>::hold(account, balance)
			.expect("should not fail to hold existing tokens");
	} else {
		<orml_tokens::Pallet<T> as fungibles::MutateHold<T::AccountId>>::hold(
			currency_id,
			account,
			balance,
		)
		.expect("should not fail to hold existing tokens");
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
	T: Config + pallet_permissions::Config<Location = PoolId, Role = PoolRole>,
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
	T: frame_system::Config + pallet_permissions::Config<Location = PoolId, Role = PoolRole>,
{
	<pallet_permissions::Pallet<T> as Permissions<T::AccountId>>::add(
		pool_id,
		acc,
		PoolRole::TrancheInvestor(tranche_id, u64::MAX),
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
		+ pallet_balances::Config<Balance = <T as Config>::Balance>
		+ orml_tokens::Config<
			Balance = <T as Config>::Balance,
			CurrencyId = <T as Config>::CurrencyId,
		> + pallet_permissions::Config<Location = PoolId, Role = PoolRole>,
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

static mut COUNTER: u32 = 0u32;
// TODO: Make this actually random. rand-crate is not suitable as the features clash with substrate
//       and wasm.
fn get_random_non_native_id<T>() -> T::CurrencyId
where
	T: Config,
	T::CurrencyId: From<CurrencyId>,
{
	// A match call just to ensure we increase the u32 below in case the number
	// of enum variants change.
	//
	// NOTE: We do not want to be CurrencyId::Native to be used here
	let max_variants = match CurrencyId::Native {
		CurrencyId::Native => 2u32,
		CurrencyId::Tranche(_, _) => {
			unreachable!("We only want the max_used_variants count to be returned. qed.")
		}
		CurrencyId::Usd => {
			unreachable!("We only want the max_used_variants count to be returned. qed.")
		}
	};

	let curr = unsafe {
		let curr = COUNTER;
		COUNTER = (COUNTER + 1) % max_variants;
		curr
	};

	match curr {
		_x if curr == 0 => CurrencyId::Tranche(0, 0).into(),
		_x if curr == 1 => CurrencyId::Usd.into(),
		_ => unreachable!("We only want the range of enum discrimants to be covered. qed."),
	}
}

benchmarks! {
	where_clause {
		where
		T: Config
			+ pallet_balances::Config<Balance = <T as Config>::Balance>
			+ orml_tokens::Config<Balance = <T as Config>::Balance, CurrencyId = <T as Config>::CurrencyId>
			+ pallet_permissions::Config<Location = PoolId, Role = PoolRole>,
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
		let currency = get_random_non_native_id::<T>();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:transfer(RawOrigin::Signed(send.clone()), recv_loopup, currency.clone(), amount)
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, false) == amount);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &send, false) == Zero::zero());
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
		let currency = get_random_non_native_id::<T>();
		let min_deposit = <T as orml_tokens::Config>::ExistentialDeposits::get(&currency);
		let send_amount = amount - min_deposit;
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:transfer_keep_alive(RawOrigin::Signed(send.clone()), recv_loopup, currency, send_amount)
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, false)  == send_amount);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &send, false)  == amount - send_amount);
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
	}:transfer_all(RawOrigin::Signed(send.clone()), recv_loopup, currency, false)
	verify {
		assert!(pallet_balances::Pallet::<T>::free_balance(&recv) == amount);
		assert!(pallet_balances::Pallet::<T>::free_balance(&send) == Zero::zero());
	}

	// We benchmark into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	// We let the other die to have clean-up logic in weight
	transfer_all_other {
		let amount = as_balance::<T>(300);
		let currency: <T as Config>::CurrencyId = get_random_non_native_id::<T>();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:transfer_all(RawOrigin::Signed(send.clone()), recv_loopup, currency.clone(), false)
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, false) == amount);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &send, false) == Zero::zero());
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
		let currency: <T as Config>::CurrencyId = get_random_non_native_id::<T>();
		let send = set_up_account::<T>("sender", currency.clone(), amount, None);
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let send_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(send.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:force_transfer(RawOrigin::Root, send_loopup, recv_loopup, currency.clone(), amount)
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, false) == amount);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &send, false) == Zero::zero());
	}

	// We transfer into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	// We let the other die to have clean-up logic in weight
	set_balance_native {
		let free = as_balance::<T>(300);
		let reserved = as_balance::<T>(200);
		let currency: <T as Config>::CurrencyId = CurrencyId::Native.into();
		let recv = get_account::<T>("receiver", false);
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:set_balance(RawOrigin::Root, recv_loopup, currency.clone(), free, reserved)
	verify {
		assert!(<pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::reducible_balance(&recv, false) == free);
		assert!(<pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::balance(&recv) == (free + reserved));
	}

	// We benchmark into non-existing accounts in order to get worst-case scenarios
	// It might be beneficially to have a separation of cases in the future.
	// We let the other die to have clean-up logic in weight
	set_balance_other {
		let free = as_balance::<T>(300);
		let reserved = as_balance::<T>(200);
		let currency: <T as Config>::CurrencyId = get_random_non_native_id::<T>();
		let recv = get_account_maybe_permission::<T>("receiver", currency.clone());
		let recv_loopup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(recv.clone());
	}:set_balance(RawOrigin::Root, recv_loopup, currency.clone(), free, reserved)
	verify {
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(currency, &recv, false) == free);
		assert!(<orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::balance(currency, &recv) == (free + reserved));
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::MockRuntime,
);
