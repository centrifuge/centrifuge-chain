// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Module provides loan related functions
use super::*;
use crate::weights::WeightInfo;
use common_types::{Adjustment, PoolLocator};
use frame_support::weights::Weight;
use sp_runtime::ArithmeticError;

impl<T: Config> Pallet<T> {
	// calculates write off group weight for count number of write off groups looped
	// this function needs to adjusted when the reads and write changes for the write off group extrinsic
	pub(crate) fn write_off_group_weight(count: u64) -> Weight {
		T::WeightInfo::write_off()
			.saturating_mul(count)
			.saturating_sub(
				(count - 1).saturating_mul(
					T::DbWeight::get()
						.reads(4)
						.saturating_add(T::DbWeight::get().writes(2)),
				),
			)
	}

	/// returns the account_id of the loan pallet
	pub fn account_id() -> T::AccountId {
		T::LoansPalletId::get().into_account()
	}

	/// check if the given loan belongs to the owner provided
	pub(crate) fn check_loan_owner(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		expected_owner: T::AccountId,
	) -> Result<AssetOf<T>, DispatchError> {
		let loan_class_id =
			PoolToLoanNftClass::<T>::get(pool_id).ok_or(Error::<T>::PoolNotInitialised)?;

		let actual_owner = T::NonFungible::owner(&loan_class_id.into(), &loan_id.into())
			.ok_or(Error::<T>::NFTOwnerNotFound)?;
		ensure!(actual_owner == expected_owner, Error::<T>::NotAssetOwner);

		Ok(Asset(loan_class_id, loan_id))
	}

	/// issues a new loan nft and returns the LoanID
	pub(crate) fn create_loan(
		pool_id: PoolIdOf<T>,
		collateral_owner: T::AccountId,
		collateral: AssetOf<T>,
	) -> Result<T::LoanId, sp_runtime::DispatchError> {
		// check if the nft belongs to owner
		let (collateral_class_id, instance_id) = collateral.destruct();
		let owner = T::NonFungible::owner(&collateral_class_id.into(), &instance_id.into())
			.ok_or(Error::<T>::NFTOwnerNotFound)?;
		ensure!(owner == collateral_owner, Error::<T>::NotAssetOwner);

		// check if the registry is not an loan nft registry
		ensure!(
			!LoanNftClassToPool::<T>::contains_key(collateral_class_id),
			Error::<T>::NotAValidAsset
		);

		// create new loan nft
		let nonce = NextLoanId::<T>::get(pool_id);
		let loan_id: T::LoanId = nonce.into();
		let loan_class_id =
			PoolToLoanNftClass::<T>::get(pool_id).ok_or(Error::<T>::PoolNotInitialised)?;
		T::NonFungible::mint_into(&loan_class_id.into(), &loan_id.into(), &owner)?;

		// lock collateral nft
		let pool_account = PoolLocator { pool_id }.into_account();
		T::NonFungible::transfer(
			&collateral_class_id.into(),
			&instance_id.into(),
			&pool_account,
		)?;

		// update the next token nonce
		let next_loan_id = nonce
			.checked_add(1)
			.ok_or(Error::<T>::NftTokenNonceOverflowed)?;
		NextLoanId::<T>::mutate(pool_id, |loan_id| *loan_id = next_loan_id);

		// create loan
		Loan::<T>::insert(
			pool_id,
			loan_id,
			LoanDetails {
				collateral,
				loan_type: Default::default(),
				status: LoanStatus::Created,
				interest_rate_per_sec: Zero::zero(),
				origination_date: None,
				normalized_debt: Zero::zero(),
				total_borrowed: Zero::zero(),
				total_repaid: Zero::zero(),
				admin_written_off: false,
				write_off_index: None,
			},
		);
		Ok(loan_id)
	}

	pub(crate) fn price_loan(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		interest_rate_per_sec: T::Rate,
		loan_type: LoanType<T::Rate, T::Balance>,
	) -> DispatchResult {
		Loan::<T>::try_mutate(pool_id, loan_id, |loan| -> DispatchResult {
			let loan = loan.as_mut().ok_or(Error::<T>::MissingLoan)?;

			// ensure loan is created or priced but not yet borrowed against
			ensure!(
				loan.status == LoanStatus::Created
					|| loan.status == LoanStatus::Active && loan.total_borrowed == Zero::zero(),
				Error::<T>::LoanIsActive
			);

			// ensure loan_type is valid
			let now = Self::now();
			ensure!(loan_type.is_valid(now), Error::<T>::LoanValueInvalid);

			// ensure interest_rate_per_sec >= one
			ensure!(
				interest_rate_per_sec >= One::one(),
				Error::<T>::LoanValueInvalid
			);

			// update the loan info
			loan.interest_rate_per_sec = interest_rate_per_sec;
			loan.status = LoanStatus::Active;
			loan.loan_type = loan_type;

			Ok(())
		})
	}

	// try to close a given loan.
	// returns the asset/collateral loan is associated with along with bool that says whether loan was completely written off.
	pub(crate) fn close_loan(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		owner: T::AccountId,
	) -> Result<ClosedLoan<T>, DispatchError> {
		// ensure owner is the loan nft owner
		let loan_nft = Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		Loan::<T>::try_mutate(
			pool_id,
			loan_id,
			|loan| -> Result<ClosedLoan<T>, DispatchError> {
				let loan = loan.as_mut().ok_or(Error::<T>::MissingLoan)?;

				// ensure loan is active
				ensure!(loan.status == LoanStatus::Active, Error::<T>::LoanNotActive);

				// ensure debt is all paid
				// we just need to ensure normalized debt is zero
				// if not, we check if the loan is written of 100%
				let written_off = match (loan.normalized_debt == Zero::zero(), loan.write_off_index)
				{
					// debt is cleared
					(true, _) => Ok(false),
					// debt not cleared and loan not written off
					(_, None) => Err(Error::<T>::LoanNotRepaid),
					// debt not cleared but loan is written off
					// if written off completely, then we can close it
					(_, Some(write_off_index)) => {
						let groups = PoolWriteOffGroups::<T>::get(pool_id);
						let group = groups
							.get(write_off_index as usize)
							.ok_or(Error::<T>::InvalidWriteOffGroupIndex)?;
						ensure!(group.percentage == One::one(), Error::<T>::LoanNotRepaid);
						Ok(true)
					}
				}?;

				// transfer collateral nft to owner
				let collateral = loan.collateral;
				let (collateral_class_id, instance_id) = collateral.destruct();
				T::NonFungible::transfer(&collateral_class_id.into(), &instance_id.into(), &owner)?;

				// burn loan nft
				let (loan_class_id, loan_id) = loan_nft.destruct();
				T::NonFungible::burn_from(&loan_class_id.into(), &loan_id.into())?;

				// update loan status
				loan.status = LoanStatus::Closed;
				Ok(ClosedLoan {
					collateral,
					written_off,
				})
			},
		)
	}

	// tries to borrow some amount on a loan that is active.
	// returns a bool indicating if this is the first borrow or not.
	pub(crate) fn borrow_amount(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		owner: T::AccountId,
		amount: T::Balance,
	) -> Result<bool, DispatchError> {
		// ensure owner is the loan owner
		Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		Loan::<T>::try_mutate(pool_id, loan_id, |loan| -> Result<bool, DispatchError> {
			let loan = loan.as_mut().ok_or(Error::<T>::MissingLoan)?;

			// ensure loan is active
			ensure!(loan.status == LoanStatus::Active, Error::<T>::LoanNotActive);

			// ensure loan is not written off
			ensure!(
				loan.write_off_index.is_none(),
				Error::<T>::WrittenOffByAdmin
			);

			// ensure maturity date has not passed if the loan has a maturity date
			let now: Moment = Self::now();
			let valid = match loan.loan_type.maturity_date() {
				Some(md) => md > now,
				None => true,
			};
			ensure!(valid, Error::<T>::LoanMaturityDatePassed);

			// ensure borrow amount is positive
			ensure!(amount > Zero::zero(), Error::<T>::LoanValueInvalid);

			// check for max borrow amount
			let old_debt =
				T::InterestAccrual::current_debt(loan.interest_rate_per_sec, loan.normalized_debt)?;

			let max_borrow_amount = loan.max_borrow_amount(old_debt);
			ensure!(
				amount <= max_borrow_amount,
				Error::<T>::MaxBorrowAmountExceeded
			);

			// get previous present value so that we can update the nav accordingly
			// we already know that that loan is not written off,
			// means we wont need to have write off groups. so save a DB read and pass empty
			let old_pv = loan
				.present_value(old_debt, &vec![], now)
				.ok_or(Error::<T>::LoanPresentValueFailed)?;

			let new_total_borrowed = loan
				.total_borrowed
				.checked_add(&amount)
				.ok_or(ArithmeticError::Overflow)?;

			// calculate new normalized debt with adjustment amount
			let normalized_debt = T::InterestAccrual::adjust_normalized_debt(
				loan.interest_rate_per_sec,
				loan.normalized_debt,
				Adjustment::Increase(amount),
			)?;

			// update loan
			let first_borrow = loan.total_borrowed == Zero::zero();

			if first_borrow {
				loan.origination_date = Some(now);
			}

			loan.total_borrowed = new_total_borrowed;
			loan.normalized_debt = normalized_debt;

			let new_debt =
				T::InterestAccrual::current_debt(loan.interest_rate_per_sec, loan.normalized_debt)?;

			let new_pv = loan
				.present_value(new_debt, &vec![], now)
				.ok_or(Error::<T>::LoanPresentValueFailed)?;
			Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;
			T::Pool::withdraw(pool_id, owner, amount)?;
			Ok(first_borrow)
		})
	}

	pub(crate) fn update_nav_with_updated_present_value(
		pool_id: PoolIdOf<T>,
		new_pv: T::Balance,
		old_pv: T::Balance,
	) -> Result<(), DispatchError> {
		// calculate new diff from the old and new present value and update the nav accordingly
		PoolNAV::<T>::try_mutate(pool_id, |maybe_nav_details| -> Result<(), DispatchError> {
			let mut nav = maybe_nav_details.take().unwrap_or_default();

			let new_nav = match new_pv > old_pv {
				// borrow
				true => new_pv
					.checked_sub(&old_pv)
					.and_then(|positive_diff| nav.latest.checked_add(&positive_diff))
					.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow)),
				// repay since new pv is less than old
				false => old_pv
					.checked_sub(&new_pv)
					.and_then(|negative_diff| nav.latest.checked_sub(&negative_diff))
					.ok_or(DispatchError::Arithmetic(ArithmeticError::Underflow)),
			}?;
			nav.latest = new_nav;
			*maybe_nav_details = Some(nav);
			Self::deposit_event(Event::<T>::NAVUpdated(
				pool_id,
				new_nav,
				NAVUpdateType::Inexact,
			));
			Ok(())
		})
	}

	pub(crate) fn repay_amount(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		owner: T::AccountId,
		amount: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		// ensure owner is the loan owner
		Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		Loan::<T>::try_mutate(
			pool_id,
			loan_id,
			|loan| -> Result<T::Balance, DispatchError> {
				let loan = loan.as_mut().ok_or(Error::<T>::MissingLoan)?;

				// ensure loan is active
				ensure!(loan.status == LoanStatus::Active, Error::<T>::LoanNotActive);

				let now: Moment = Self::now();

				// ensure current time is more than origination time
				// this is mainly to deal with how we calculate debt while trying to repay
				// therefore we do not let users repay at same instant origination happened
				ensure!(
					now > loan
						.origination_date
						.expect("Active loan should have an origination date"),
					Error::<T>::RepayTooEarly
				);

				// ensure repay amount is positive
				ensure!(amount > Zero::zero(), Error::<T>::LoanValueInvalid);

				// TODO: this should calculate debt at the last NAV update
				let old_debt = T::InterestAccrual::current_debt(
					loan.interest_rate_per_sec,
					loan.normalized_debt,
				)?;

				// calculate old present_value
				let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
				let old_pv = loan
					.present_value(old_debt, &write_off_groups, now)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;

				// ensure amount is not more than current debt
				let repay_amount = amount.min(old_debt);

				let new_total_repaid = loan
					.total_repaid
					.checked_add(&repay_amount)
					.ok_or(ArithmeticError::Overflow)?;

				// calculate new normalized debt with repaid amount
				let normalized_debt = T::InterestAccrual::adjust_normalized_debt(
					loan.interest_rate_per_sec,
					loan.normalized_debt,
					Adjustment::Decrease(repay_amount),
				)?;

				loan.total_repaid = new_total_repaid;
				loan.normalized_debt = normalized_debt;

				let new_debt = T::InterestAccrual::current_debt(
					loan.interest_rate_per_sec,
					loan.normalized_debt,
				)?;

				let new_pv = loan
					.present_value(new_debt, &write_off_groups, now)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;
				Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;
				T::Pool::deposit(pool_id, owner, repay_amount)?;
				Ok(repay_amount)
			},
		)
	}

	pub(crate) fn now() -> Moment {
		T::Time::now().as_secs()
	}

	/// accrues rate and debt of a given loan and updates it
	/// returns the present value of the loan accounting any write offs
	pub(crate) fn accrue_and_update_loan(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		write_off_groups: &Vec<WriteOffGroup<T::Rate>>,
	) -> Result<T::Balance, DispatchError> {
		Loan::<T>::try_mutate(
			pool_id,
			loan_id,
			|loan| -> Result<T::Balance, DispatchError> {
				let loan = loan.as_mut().ok_or(Error::<T>::MissingLoan)?;

				// if the loan is not active, then skip updating and return PV as zero
				if loan.status != LoanStatus::Active {
					return Ok(Zero::zero());
				}

				let debt = T::InterestAccrual::current_debt(
					loan.interest_rate_per_sec,
					loan.normalized_debt,
				)?;

				let now: Moment = Self::now();

				let present_value = loan
					.present_value(debt, write_off_groups, now)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;

				Ok(present_value)
			},
		)
	}

	/// updates nav for the given pool and returns the latest NAV at this instant and number of loans accrued.
	pub(crate) fn update_nav_of_pool(
		pool_id: PoolIdOf<T>,
	) -> Result<(T::Balance, Moment), DispatchError> {
		let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
		let mut updated_loans = 0;
		let nav = Loan::<T>::iter_key_prefix(pool_id).try_fold(
			Zero::zero(),
			|sum, loan_id| -> Result<T::Balance, DispatchError> {
				let pv = Self::accrue_and_update_loan(pool_id, loan_id, &write_off_groups)?;
				updated_loans += 1;
				sum.checked_add(&pv)
					.ok_or(Error::<T>::LoanAccrueFailed.into())
			},
		)?;
		PoolNAV::<T>::insert(
			pool_id,
			NAVDetails {
				latest: nav,
				last_updated: Self::now(),
			},
		);
		Ok((nav, updated_loans))
	}

	pub(crate) fn add_write_off_group_to_pool(
		pool_id: PoolIdOf<T>,
		group: WriteOffGroup<T::Rate>,
	) -> Result<u32, DispatchError> {
		// ensure pool is initialised
		ensure!(
			PoolToLoanNftClass::<T>::contains_key(pool_id),
			Error::<T>::PoolNotInitialised,
		);

		// ensure write off percentage is not more than 100%
		ensure!(
			group.percentage <= One::one(),
			Error::<T>::InvalidWriteOffGroup
		);

		// append new group
		let index = PoolWriteOffGroups::<T>::mutate(pool_id, |write_off_groups| -> u32 {
			write_off_groups.push(group);
			// return the index of the latest write off group
			(write_off_groups.len() - 1) as u32
		});

		Ok(index)
	}

	/// writes off a given unhealthy loan
	/// if override_write_off_index is Some, this is a admin action and loan override flag is set
	/// if loan is already overridden and override_write_off_index is None, we return error
	/// if loan is still healthy, we return an error
	/// loan is accrued and nav is updated accordingly
	/// returns new write off index applied to loan
	pub(crate) fn write_off_loan(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		override_write_off_index: Option<u32>,
	) -> Result<u32, DispatchError> {
		Loan::<T>::try_mutate(pool_id, loan_id, |loan| -> Result<u32, DispatchError> {
			let loan = loan.as_mut().ok_or(Error::<T>::MissingLoan)?;

			// ensure loan is active
			ensure!(loan.status == LoanStatus::Active, Error::<T>::LoanNotActive);

			// ensure loan was not overwritten by admin and try to fetch a valid write off group for loan
			let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
			let write_off_group_index = match override_write_off_index {
				// admin is trying to write off
				Some(index) => {
					// check if the write off group exists
					write_off_groups
						.get(index as usize)
						.ok_or(Error::<T>::InvalidWriteOffGroupIndex)?;
					loan.admin_written_off = true;
					Ok(index)
				}
				None => {
					// non-admin is trying to write off but admin already did. So error out
					if loan.admin_written_off {
						return Err(Error::<T>::WrittenOffByAdmin.into());
					}

					let maturity_date = loan
						.loan_type
						.maturity_date()
						.ok_or(Error::<T>::LoanTypeInvalid)?;

					// ensure loan's maturity date has passed
					let now = Self::now();
					ensure!(now > maturity_date, Error::<T>::LoanHealthy);

					// not written off by admin, and non admin trying to write off, then
					// fetch the best write group available for this loan
					math::valid_write_off_group(maturity_date, now, &write_off_groups)?
						.ok_or(Error::<T>::NoValidWriteOffGroup)
				}
			}?;

			let debt =
				T::InterestAccrual::current_debt(loan.interest_rate_per_sec, loan.normalized_debt)?;

			let now: Moment = Self::now();

			// get old present value accounting for any write offs
			let old_pv = loan
				.present_value(debt, &write_off_groups, now)
				.ok_or(Error::<T>::LoanPresentValueFailed)?;

			loan.write_off_index = Some(write_off_group_index);

			// calculate updated write off adjusted present value
			let new_pv = loan
				.present_value(debt, &write_off_groups, now)
				.ok_or(Error::<T>::LoanPresentValueFailed)?;

			// update nav
			Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;

			Ok(write_off_group_index)
		})
	}
}
