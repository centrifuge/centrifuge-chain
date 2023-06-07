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
//
// Much of this code is copied verbatim from Frontier
// Copyright (c) 2020-2022 Parity Technologies (UK) Ltd.

use core::marker::PhantomData;

use frame_support::traits::{
	Currency, ExistenceRequirement, Imbalance, OnUnbalanced, SignedImbalance, WithdrawReasons,
};
use pallet_balances::Pallet as Balances;
use pallet_evm::{AddressMapping, OnChargeEVMTransaction};
use pallet_treasury::Pallet as Treasury;
use sp_core::{H160, U256};
use sp_runtime::{
	traits::{UniqueSaturatedInto, Zero},
	Saturating,
};

use crate::fees::{DealWithFees, NegativeImbalance};

type AccountIdOf<R> = <R as frame_system::Config>::AccountId;
type Balance<R> = <Balances<R> as Currency<AccountIdOf<R>>>::Balance;
type PositiveImbalance<R> = <Balances<R> as Currency<AccountIdOf<R>>>::PositiveImbalance;

type Error<R> = pallet_evm::Error<R>;

/// Handler for EVM fees
pub struct DealWithEVMFees<R>(PhantomData<R>);

impl<R> OnChargeEVMTransaction<R> for DealWithEVMFees<R>
where
	R: pallet_balances::Config
		+ pallet_treasury::Config
		+ pallet_authorship::Config
		+ pallet_evm::Config,
	Treasury<R>: OnUnbalanced<NegativeImbalance<R>>,
	Balances<R>: Currency<<R as frame_system::Config>::AccountId>,
	U256: UniqueSaturatedInto<Balance<R>>,
{
	type LiquidityInfo = Option<NegativeImbalance<R>>;

	fn withdraw_fee(who: &H160, fee: U256) -> Result<Self::LiquidityInfo, Error<R>> {
		if fee.is_zero() {
			return Ok(None);
		}
		let account_id = R::AddressMapping::into_account_id(*who);
		Balances::<R>::withdraw(
			&account_id,
			fee.unique_saturated_into(),
			WithdrawReasons::FEE,
			ExistenceRequirement::AllowDeath,
		)
		.map(Some)
		.map_err(|_| Error::<R>::BalanceLow)
	}

	fn correct_and_deposit_fee(
		who: &H160,
		corrected_fee: U256,
		base_fee: U256,
		already_withdrawn: Self::LiquidityInfo,
	) -> Self::LiquidityInfo {
		let Some(paid) = already_withdrawn else { return None };

		let account_id = R::AddressMapping::into_account_id(*who);

		// Calculate how much refund we should return
		let refund_amount = paid
			.peek()
			.saturating_sub(corrected_fee.unique_saturated_into());
		// refund to the account that paid the fees. If this fails, the
		// account might have dropped below the existential balance. In
		// that case we don't refund anything.
		let refund_imbalance = Balances::<R>::deposit_into_existing(&account_id, refund_amount)
			.unwrap_or_else(|_| PositiveImbalance::<R>::zero());

		// Make sure this works with 0 ExistentialDeposit
		// https://github.com/paritytech/substrate/issues/10117
		// If we tried to refund something, the account still empty and the ED is set to
		// 0, we call `make_free_balance_be` with the refunded amount.
		let refund_imbalance = if Balances::<R>::minimum_balance().is_zero()
			&& refund_amount > <Balance<R>>::zero()
			&& Balances::<R>::total_balance(&account_id).is_zero()
		{
			// Known bug: Substrate tried to refund to a zeroed AccountData, but
			// interpreted the account to not exist.
			match Balances::<R>::make_free_balance_be(&account_id, refund_amount) {
				SignedImbalance::Positive(p) => p,
				_ => PositiveImbalance::<R>::zero(),
			}
		} else {
			refund_imbalance
		};

		// merge the imbalance caused by paying the fees and refunding parts of it
		// again.
		let adjusted_paid = paid
			.offset(refund_imbalance)
			.same()
			.unwrap_or_else(|_| NegativeImbalance::<R>::zero());

		let (base_fee, tip) = adjusted_paid.split(base_fee.unique_saturated_into());
		DealWithFees::<R>::on_unbalanceds([base_fee, tip].into_iter());
		None
	}

	fn pay_priority_fee(tip: Self::LiquidityInfo) {
		// Because we handle the priority fee in the above function,
		// there's nothing to do here. Assert for verification
		// purposes.
		assert!(tip.is_none())
	}
}
