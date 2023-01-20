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
use cfg_types::adjustments::Adjustment;
use pallet_pool_system::pool_types::PoolLocator;
use sp_runtime::{traits::BadOrigin, ArithmeticError};

use super::*;

impl<T: Config> Pallet<T> {
	/// returns the account_id of the loan pallet
	pub fn account_id() -> T::AccountId {
		T::LoansPalletId::get().into_account_truncating()
	}

	pub fn ensure_role(
		pool_id: PoolIdOf<T>,
		sender: T::AccountId,
		role: PoolRole,
	) -> Result<(), BadOrigin> {
		T::Permission::has(PermissionScope::Pool(pool_id), sender, Role::PoolRole(role))
			.then(|| ())
			.ok_or(BadOrigin)
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

	// TODO: Move this to test-utils
	#[cfg(any(test, feature = "runtime-benchmarks"))]
	pub(crate) fn get_active_loan(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
	) -> Option<PricedLoanDetailsOf<T>> {
		let active_loans = ActiveLoans::<T>::get(pool_id);

		active_loans
			.into_iter()
			.find(|active_loan| active_loan.loan_id == loan_id)
	}

	pub(crate) fn try_mutate_active_loan<R, F>(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		f: F,
	) -> Result<(ActiveCount, R), DispatchError>
	where
		F: FnOnce(&mut PricedLoanDetailsOf<T>) -> Result<R, DispatchError>,
	{
		ActiveLoans::<T>::try_mutate(
			pool_id,
			|active_loans| -> Result<(ActiveCount, R), DispatchError> {
				let len = active_loans.len().try_into().unwrap();
				for active_loan_option in active_loans.iter_mut() {
					if active_loan_option.loan_id == loan_id {
						return f(active_loan_option).map(|r| (len, r));
					}
				}

				Err(Error::<T>::MissingLoan.into())
			},
		)
	}

	pub(crate) fn rate_with_penalty(
		loan: &PricedLoanDetailsOf<T>,
		write_off_groups: &[WriteOffGroup<T::Rate>],
	) -> T::Rate {
		match loan.write_off_status {
			WriteOffStatus::None => loan.interest_rate_per_sec,
			WriteOffStatus::WrittenOff { write_off_index } => {
				loan.interest_rate_per_sec
					+ write_off_groups[write_off_index as usize].penalty_interest_rate_per_sec
			}
			WriteOffStatus::WrittenOffByAdmin {
				penalty_interest_rate_per_sec,
				..
			} => loan.interest_rate_per_sec + penalty_interest_rate_per_sec,
		}
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
		let pool_account = PoolLocator { pool_id }.into_account_truncating();
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
				status: LoanStatus::Created,
			},
		);
		Ok(loan_id)
	}

	pub(crate) fn price_created_loan(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		interest_rate_per_year: T::Rate,
		loan_type: LoanType<T::Rate, T::Balance>,
	) -> Result<(u32, T::Rate), DispatchError> {
		let now = Self::now();
		ensure!(loan_type.is_valid(now), Error::<T>::LoanValueInvalid);

		let interest_rate_per_sec =
			T::InterestAccrual::reference_yearly_rate(interest_rate_per_year)?;
		let active_loan = PricedLoanDetails {
			loan_id,
			loan_type,
			interest_rate_per_sec,
			origination_date: None,
			normalized_debt: Zero::zero(),
			total_borrowed: Zero::zero(),
			total_repaid: Zero::zero(),
			write_off_status: WriteOffStatus::None,
			last_updated: now,
		};

		let mut active_loans = ActiveLoans::<T>::get(pool_id);
		active_loans
			.try_push(active_loan)
			.map_err(|_| Error::<T>::TooManyActiveLoans)?;
		let count = active_loans.len();
		ActiveLoans::<T>::insert(pool_id, active_loans);

		Ok((
			count
				.try_into()
				.expect("len is 32-bit in WASM, this cannot panic"),
			interest_rate_per_sec,
		))
	}

	pub(crate) fn price_active_loan(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		interest_rate_per_year: T::Rate,
		loan_type: LoanType<T::Rate, T::Balance>,
	) -> Result<(u32, T::Rate), DispatchError> {
		let now = Self::now();
		ensure!(loan_type.is_valid(now), Error::<T>::LoanValueInvalid);

		let interest_rate_per_sec =
			T::InterestAccrual::reference_yearly_rate(interest_rate_per_year)?;
		Self::try_mutate_active_loan(
			pool_id,
			loan_id,
			|active_loan| -> Result<(), DispatchError> {
				let old_debt = T::InterestAccrual::previous_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
					active_loan.last_updated,
				)?;

				// calculate old present_value
				let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
				let old_pv = active_loan
					.present_value(old_debt, &write_off_groups, active_loan.last_updated)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;

				// calculate new normalized debt without amount
				let normalized_debt = T::InterestAccrual::renormalize_debt(
					active_loan.interest_rate_per_sec,
					interest_rate_per_sec,
					active_loan.normalized_debt,
				)?;

				T::InterestAccrual::unreference_rate(active_loan.interest_rate_per_sec)?;

				active_loan.loan_type = loan_type;
				active_loan.interest_rate_per_sec = interest_rate_per_sec;
				active_loan.normalized_debt = normalized_debt;
				active_loan.last_updated = now;

				let new_debt = T::InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)?;

				// calculate new present_value
				let new_pv = active_loan
					.present_value(new_debt, &write_off_groups, now)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;

				Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)
			},
		)?;

		Ok((
			ActiveLoans::<T>::get(pool_id).len().try_into().unwrap(),
			interest_rate_per_sec,
		))
	}

	// try to close a given loan.
	// returns the asset/collateral loan is associated with along with bool that says whether loan was completely written off.
	pub(crate) fn close_loan(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		owner: T::AccountId,
	) -> Result<(ActiveCount, ClosedLoan<T>), DispatchError> {
		// ensure owner is the loan nft owner
		let loan_nft = Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		let collateral = Loan::<T>::try_mutate(
			pool_id,
			loan_id,
			|loan| -> Result<AssetOf<T>, DispatchError> {
				let loan = loan.as_mut().ok_or(Error::<T>::MissingLoan)?;

				// ensure loan is active
				ensure!(loan.status == LoanStatus::Active, Error::<T>::LoanNotActive);

				// update loan status
				let closed_at = T::BlockNumberProvider::current_block_number();
				loan.status = LoanStatus::Closed { closed_at };
				Ok(loan.collateral)
			},
		)?;

		ActiveLoans::<T>::try_mutate(
			pool_id,
			|active_loans| -> Result<(ActiveCount, ClosedLoan<T>), DispatchError> {
				let (active_loan_idx, active_loan) = active_loans
					.iter()
					.enumerate()
					.find(|(_, active_loan)| active_loan.loan_id == loan_id)
					.ok_or(Error::<T>::MissingLoan)?;

				// ensure debt is all paid
				// we just need to ensure normalized debt is zero
				// if not, we check if the loan is written of 100%
				let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
				let written_off = match (
					active_loan.normalized_debt == Zero::zero(),
					active_loan.write_off_status,
				) {
					// debt is cleared
					(true, _) => Ok(false),
					// debt not cleared and loan not written off
					(_, WriteOffStatus::None) => Err(Error::<T>::LoanNotRepaid),
					// debt not cleared but loan is written off
					// if written off completely, then we can close it
					(_, WriteOffStatus::WrittenOff { write_off_index }) => {
						let group = write_off_groups
							.get(write_off_index as usize)
							.ok_or(Error::<T>::InvalidWriteOffGroupIndex)?;
						ensure!(group.percentage == One::one(), Error::<T>::LoanNotRepaid);
						Ok(true)
					}
					// debt not cleared but loan is written off by admin
					// if written off completely, then we can close it
					(_, WriteOffStatus::WrittenOffByAdmin { percentage, .. }) => {
						ensure!(percentage == One::one(), Error::<T>::LoanNotRepaid);
						Ok(true)
					}
				}?;

				let interest_rate_with_penalty =
					Self::rate_with_penalty(active_loan, &write_off_groups);

				// transfer collateral nft to owner
				let (collateral_class_id, instance_id) = collateral.destruct();
				T::NonFungible::transfer(&collateral_class_id.into(), &instance_id.into(), &owner)?;

				// burn loan nft
				let (loan_class_id, loan_id) = loan_nft.destruct();
				T::NonFungible::burn(&loan_class_id.into(), &loan_id.into(), None)?;

				T::InterestAccrual::unreference_rate(interest_rate_with_penalty)?;
				let active_count = active_loans.len();
				let closed_loan = active_loans.remove(active_loan_idx);
				ClosedLoans::<T>::insert(pool_id, loan_id, closed_loan);

				Ok((
					active_count.try_into().unwrap(),
					ClosedLoan {
						collateral,
						written_off,
					},
				))
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
	) -> Result<(ActiveCount, bool), DispatchError> {
		// ensure owner is the loan owner
		Self::check_loan_owner(pool_id, loan_id, owner.clone())?;
		let loan = Loan::<T>::get(pool_id, loan_id).ok_or(Error::<T>::MissingLoan)?;
		ensure!(loan.status == LoanStatus::Active, Error::<T>::LoanNotActive);

		Self::try_mutate_active_loan(
			pool_id,
			loan_id,
			|active_loan| -> Result<bool, DispatchError> {
				// ensure loan is not written off
				ensure!(
					active_loan.write_off_status == WriteOffStatus::None,
					Error::<T>::WrittenOffByAdmin
				);

				// ensure maturity date has not passed if the loan has a maturity date
				let now: Moment = Self::now();
				let valid = match active_loan.loan_type.maturity_date() {
					Some(md) => md > now,
					None => true,
				};
				ensure!(valid, Error::<T>::LoanMaturityDatePassed);

				// ensure borrow amount is positive
				ensure!(amount > Zero::zero(), Error::<T>::LoanValueInvalid);

				// check for max borrow amount
				let old_debt = T::InterestAccrual::previous_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
					active_loan.last_updated,
				)?;

				let current_debt = T::InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)?;

				let max_borrow_amount = active_loan.max_borrow_amount(current_debt);
				ensure!(
					amount <= max_borrow_amount,
					Error::<T>::MaxBorrowAmountExceeded
				);

				// get previous present value so that we can update the nav accordingly
				// we already know that that loan is not written off,
				// means we wont need to have write off groups. so save a DB read and pass empty
				let old_pv = active_loan
					.present_value(old_debt, &[], active_loan.last_updated)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;

				let new_total_borrowed = active_loan
					.total_borrowed
					.checked_add(&amount)
					.ok_or(ArithmeticError::Overflow)?;

				// calculate new normalized debt with adjustment amount
				let normalized_debt = T::InterestAccrual::adjust_normalized_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
					Adjustment::Increase(amount),
				)?;

				// update loan
				let first_borrow = active_loan.total_borrowed == Zero::zero();

				if first_borrow {
					active_loan.origination_date = Some(now);
				}

				active_loan.total_borrowed = new_total_borrowed;
				active_loan.normalized_debt = normalized_debt;
				active_loan.last_updated = now;

				let new_debt = T::InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)?;

				let new_pv = active_loan
					.present_value(new_debt, &[], now)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;
				Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;
				T::Pool::withdraw(pool_id, owner, amount)?;
				Ok(first_borrow)
			},
		)
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
				false => Ok(old_pv
					.checked_sub(&new_pv)
					.and_then(|negative_diff| nav.latest.checked_sub(&negative_diff))
					.unwrap_or_else(Zero::zero)),
			}?;
			nav.latest = new_nav;
			*maybe_nav_details = Some(nav);
			Self::deposit_event(Event::<T>::NAVUpdated {
				pool_id,
				nav: new_nav,
				update_type: NAVUpdateType::Inexact,
			});
			Ok(())
		})
	}

	pub(crate) fn repay_amount(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		owner: T::AccountId,
		amount: T::Balance,
	) -> Result<(ActiveCount, T::Balance), DispatchError> {
		// ensure owner is the loan owner
		Self::check_loan_owner(pool_id, loan_id, owner.clone())?;
		let loan = Loan::<T>::get(pool_id, loan_id).ok_or(Error::<T>::MissingLoan)?;
		ensure!(loan.status == LoanStatus::Active, Error::<T>::LoanNotActive);

		Self::try_mutate_active_loan(
			pool_id,
			loan_id,
			|active_loan| -> Result<T::Balance, DispatchError> {
				let now: Moment = Self::now();

				// ensure current time is more than origination time
				// this is mainly to deal with how we calculate debt while trying to repay
				// therefore we do not let users repay at same instant origination happened
				ensure!(
					now > active_loan
						.origination_date
						.expect("Active loan should have an origination date"),
					Error::<T>::RepayTooEarly
				);

				// ensure repay amount is positive
				ensure!(amount > Zero::zero(), Error::<T>::LoanValueInvalid);

				let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
				let interest_rate_with_penalty =
					Self::rate_with_penalty(active_loan, &write_off_groups);

				let old_debt = T::InterestAccrual::previous_debt(
					interest_rate_with_penalty,
					active_loan.normalized_debt,
					active_loan.last_updated,
				)?;

				// calculate old present_value
				let old_pv = active_loan
					.present_value(old_debt, &write_off_groups, active_loan.last_updated)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;

				let current_debt = T::InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)?;

				// ensure amount is not more than current debt
				let repay_amount = amount.min(current_debt);

				let new_total_repaid = active_loan
					.total_repaid
					.checked_add(&repay_amount)
					.ok_or(ArithmeticError::Overflow)?;

				// calculate new normalized debt with repaid amount
				let normalized_debt = T::InterestAccrual::adjust_normalized_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
					Adjustment::Decrease(repay_amount),
				)?;

				active_loan.total_repaid = new_total_repaid;
				active_loan.normalized_debt = normalized_debt;
				active_loan.last_updated = now;

				let new_debt = T::InterestAccrual::current_debt(
					interest_rate_with_penalty,
					active_loan.normalized_debt,
				)?;

				let new_pv = active_loan
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
	pub(crate) fn accrue_debt_and_calculate_present_value(
		active_loan: &mut PricedLoanDetailsOf<T>,
		write_off_groups: &[WriteOffGroup<T::Rate>],
	) -> Result<T::Balance, DispatchError> {
		let interest_rate_with_penalty = Self::rate_with_penalty(active_loan, write_off_groups);

		let debt = T::InterestAccrual::current_debt(
			interest_rate_with_penalty,
			active_loan.normalized_debt,
		)?;

		let now: Moment = Self::now();
		active_loan.last_updated = now;

		let present_value = active_loan
			.present_value(debt, write_off_groups, now)
			.ok_or(Error::<T>::LoanPresentValueFailed)?;

		Ok(present_value)
	}

	/// updates nav for the given pool and returns the latest NAV at this instant and number of loans accrued.
	pub fn update_nav_of_pool(
		pool_id: PoolIdOf<T>,
	) -> Result<(ActiveCount, T::Balance), DispatchError> {
		let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);

		ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
			// Loop over all loans and sum all present values, to calculate the Net Asset Value (NAV)
			let nav = active_loans.iter_mut().try_fold(
				Zero::zero(),
				|sum, active_loan| -> Result<T::Balance, DispatchError> {
					let present_value = Self::accrue_debt_and_calculate_present_value(
						active_loan,
						&write_off_groups,
					)?;

					sum.checked_add(&present_value)
						.ok_or(sp_runtime::DispatchError::Arithmetic(
							ArithmeticError::Overflow,
						))
				},
			)?;

			// Store the latest NAV
			PoolNAV::<T>::insert(
				pool_id,
				NAVDetails {
					latest: nav,
					last_updated: Self::now(),
				},
			);
			Ok((active_loans.len().try_into().unwrap(), nav))
		})
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
		PoolWriteOffGroups::<T>::mutate(pool_id, |write_off_groups| {
			write_off_groups
				.try_push(group)
				.map(|_| (write_off_groups.len() - 1) as u32)
				.map_err(|_| Error::<T>::TooManyWriteOffGroups.into())
		})
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
		action: WriteOffAction<T::Rate>,
	) -> Result<(ActiveCount, WriteOffDetailsOf<T>), DispatchError> {
		let loan = Loan::<T>::get(pool_id, loan_id).ok_or(Error::<T>::MissingLoan)?;
		ensure!(loan.status == LoanStatus::Active, Error::<T>::LoanNotActive);

		Self::try_mutate_active_loan(
			pool_id,
			loan_id,
			|active_loan| -> Result<WriteOffDetailsOf<T>, DispatchError> {
				let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
				let (
					write_off_group_index,
					write_off_percentage,
					write_off_penalty_rate,
					new_write_off_status,
				) = match action {
					WriteOffAction::WriteOffToCurrentGroup => {
						// Loans that were already written off by an admin,
						// cannot be written off to the current group anymore.
						let is_written_off_by_admin = matches!(
							active_loan.write_off_status,
							WriteOffStatus::WrittenOffByAdmin { .. }
						);
						ensure!(!is_written_off_by_admin, Error::<T>::WrittenOffByAdmin);

						let maturity_date = active_loan
							.loan_type
							.maturity_date()
							.ok_or(Error::<T>::LoanTypeInvalid)?;

						// ensure loan's maturity date has passed
						let now = Self::now();
						ensure!(now > maturity_date, Error::<T>::LoanHealthy);

						// not written off by admin, and non admin trying to write off, then
						// fetch the best write group available for this loan
						let (write_off_index, group) =
							math::valid_write_off_group(maturity_date, now, &write_off_groups)?
								.ok_or(Error::<T>::NoValidWriteOffGroup)?;

						(
							Some(write_off_index),
							group.percentage,
							group.penalty_interest_rate_per_sec,
							WriteOffStatus::WrittenOff { write_off_index },
						)
					}
					WriteOffAction::WriteOffAsAdmin {
						percentage,
						penalty_interest_rate_per_sec,
					} => (
						None,
						percentage,
						penalty_interest_rate_per_sec,
						WriteOffStatus::WrittenOffByAdmin {
							percentage,
							penalty_interest_rate_per_sec,
						},
					),
				};

				let previous_interest_rate =
					Self::rate_with_penalty(active_loan, &write_off_groups);

				let debt = T::InterestAccrual::current_debt(
					previous_interest_rate,
					active_loan.normalized_debt,
				)?;
				let old_debt = T::InterestAccrual::previous_debt(
					previous_interest_rate,
					active_loan.normalized_debt,
					active_loan.last_updated,
				)?;

				let now: Moment = Self::now();

				// get old present value accounting for any write offs
				let old_pv = active_loan
					.present_value(old_debt, &write_off_groups, active_loan.last_updated)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;

				active_loan.write_off_status = new_write_off_status;
				active_loan.last_updated = now;

				// calculate updated write off adjusted present value
				let new_pv = active_loan
					.present_value(debt, &write_off_groups, now)
					.ok_or(Error::<T>::LoanPresentValueFailed)?;

				// Migrate written-off loan to new interest rate
				let interest_rate_with_penalty = active_loan
					.interest_rate_per_sec
					.checked_add(&write_off_penalty_rate)
					.ok_or(ArithmeticError::Overflow)?;
				T::InterestAccrual::reference_rate(interest_rate_with_penalty)?;
				active_loan.normalized_debt = T::InterestAccrual::renormalize_debt(
					previous_interest_rate,
					interest_rate_with_penalty,
					active_loan.normalized_debt,
				)?;
				T::InterestAccrual::unreference_rate(previous_interest_rate)?;

				// update nav
				Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;

				Ok((
					write_off_group_index,
					write_off_percentage,
					write_off_penalty_rate,
				))
			},
		)
	}

	/// Get the maximum amount one can borrow from a given loan in a given pool.
	/// The loan must be active and has not been written off. In addition,
	/// the maturity date must not have passed (if the loan has a maturity date).
	pub fn get_max_borrow_amount(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
	) -> Result<T::Balance, DispatchError> {
		let now = Self::now();
		let active_loans = ActiveLoans::<T>::get(pool_id);
		let active_loan = active_loans
			.into_iter()
			.find(|active_loan| active_loan.loan_id == loan_id)
			.ok_or(Error::<T>::LoanNotActive)?;

		// ensure loan is not written off
		ensure!(
			active_loan.write_off_status == WriteOffStatus::None,
			Error::<T>::WrittenOffByAdmin
		);

		// make sure maturity date has not passed if the loan has a maturity date
		let valid = match active_loan.loan_type.maturity_date() {
			Some(md) => md > now,
			None => true,
		};

		ensure!(valid, Error::<T>::LoanMaturityDatePassed);

		let current_debt = T::InterestAccrual::current_debt(
			active_loan.interest_rate_per_sec,
			active_loan.normalized_debt,
		)?;

		Ok(active_loan.max_borrow_amount(current_debt))
	}
}
