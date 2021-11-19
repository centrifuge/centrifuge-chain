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

//! module provides loan related functions
use super::*;

impl<T: Config> Pallet<T> {
	/// returns the account_id of the loan pallet
	pub fn account_id() -> T::AccountId {
		T::LoanPalletId::get().into_account()
	}

	/// check if the given loan belongs to the owner provided
	pub(crate) fn check_loan_owner(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		owner: T::AccountId,
	) -> Result<AssetOf<T>, DispatchError> {
		let loan_class_id =
			PoolToLoanNftClass::<T>::get(pool_id).ok_or(Error::<T>::ErrPoolNotInitialised)?;
		let got = T::NonFungible::owner(&loan_class_id.into(), &loan_id.into())
			.ok_or(Error::<T>::ErrNFTOwnerNotFound)?;
		ensure!(got == owner, Error::<T>::ErrNotAssetOwner);
		Ok(Asset(loan_class_id, loan_id))
	}

	/// issues a new loan nft and returns the LoanID
	pub(crate) fn issue(
		pool_id: PoolIdOf<T>,
		asset_owner: T::AccountId,
		asset: AssetOf<T>,
	) -> Result<T::LoanId, sp_runtime::DispatchError> {
		// check if the nft belongs to owner
		let (asset_class_id, instance_id) = asset.destruct();
		let owner = T::NonFungible::owner(&asset_class_id.into(), &instance_id.into())
			.ok_or(Error::<T>::ErrNFTOwnerNotFound)?;
		ensure!(owner == asset_owner, Error::<T>::ErrNotAssetOwner);

		// check if the registry is not an loan nft registry
		ensure!(
			!LoanNftClassToPool::<T>::contains_key(asset_class_id),
			Error::<T>::ErrNotAValidAsset
		);

		// create new loan nft
		let loan_pallet_account: T::AccountId = T::LoanPalletId::get().into_account();
		let nonce = NextLoanId::<T>::get();
		let loan_id: T::LoanId = nonce.into();
		let loan_class_id =
			PoolToLoanNftClass::<T>::get(pool_id).ok_or(Error::<T>::ErrPoolNotInitialised)?;
		T::NonFungible::mint_into(&loan_class_id.into(), &loan_id.into(), &owner)?;

		// lock asset nft
		T::NonFungible::transfer(
			&asset_class_id.into(),
			&instance_id.into(),
			&loan_pallet_account,
		)?;
		let timestamp = Self::time_now()?;

		// update the next token nonce
		let next_loan_id = nonce
			.checked_add(1)
			.ok_or(Error::<T>::ErrNftTokenNonceOverflowed)?;
		NextLoanId::<T>::set(next_loan_id);

		// create loan info
		LoanInfo::<T>::insert(
			pool_id,
			loan_id,
			LoanData {
				ceiling: Zero::zero(),
				borrowed_amount: Zero::zero(),
				rate_per_sec: Zero::zero(),
				accumulated_rate: One::one(),
				principal_debt: Zero::zero(),
				last_updated: timestamp,
				status: LoanStatus::Issued,
				loan_type: Default::default(),
				admin_written_off: false,
				write_off_index: None,
				asset,
			},
		);
		Ok(loan_id)
	}

	pub(crate) fn activate(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		rate_per_sec: T::Rate,
		loan_type: LoanType<T::Rate, T::Amount>,
	) -> DispatchResult {
		LoanInfo::<T>::try_mutate(pool_id, loan_id, |maybe_loan_info| -> DispatchResult {
			let mut loan_info = maybe_loan_info.take().ok_or(Error::<T>::ErrMissingLoan)?;

			// ensure loan is issued
			ensure!(
				loan_info.status == LoanStatus::Issued,
				Error::<T>::ErrLoanIsActive
			);

			// ensure loan_type is valid
			let now = Self::time_now()?;
			ensure!(loan_type.is_valid(now), Error::<T>::ErrLoanValueInvalid);

			// ensure rate_per_sec >= one
			ensure!(rate_per_sec >= One::one(), Error::<T>::ErrLoanValueInvalid);

			// calculate ceiling
			let ceiling = loan_type.ceiling().ok_or(Error::<T>::ErrLoanTypeInvalid)?;
			ensure!(ceiling > Zero::zero(), Error::<T>::ErrLoanValueInvalid);

			// update the loan info
			loan_info.rate_per_sec = rate_per_sec;
			loan_info.ceiling = ceiling;
			loan_info.status = LoanStatus::Active;
			loan_info.loan_type = loan_type;
			*maybe_loan_info = Some(loan_info);

			Ok(())
		})
	}

	pub(crate) fn close(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		owner: T::AccountId,
	) -> Result<AssetOf<T>, DispatchError> {
		// ensure owner is the loan nft owner
		let loan_nft = Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		LoanInfo::<T>::try_mutate(
			pool_id,
			loan_id,
			|maybe_loan_info| -> Result<AssetOf<T>, DispatchError> {
				let mut loan_info = maybe_loan_info.take().ok_or(Error::<T>::ErrMissingLoan)?;

				// ensure loan is active
				ensure!(
					loan_info.status == LoanStatus::Active,
					Error::<T>::ErrLoanNotActive
				);

				// ensure debt is all paid
				// we just need to ensure principal debt is zero
				// if not, we check if the loan is written of 100%
				match (
					loan_info.principal_debt == Zero::zero(),
					loan_info.write_off_index,
				) {
					// debt is cleared
					(true, _) => Ok(()),
					// debt not cleared and loan not written off
					(_, None) => Err(Error::<T>::ErrLoanNotRepaid),
					// debt not cleared but loan is written off
					// if written off completely, then we can close it
					(_, Some(write_off_index)) => {
						let groups = PoolWriteOffGroups::<T>::get(pool_id);
						let group = groups
							.get(write_off_index as usize)
							.ok_or(Error::<T>::ErrInvalidWriteOffGroupIndex)?;
						ensure!(group.percentage == One::one(), Error::<T>::ErrLoanNotRepaid);
						Ok(())
					}
				}?;

				// transfer asset to owner
				let asset = loan_info.asset;
				let (asset_class_id, instance_id) = asset.destruct();
				T::NonFungible::transfer(&asset_class_id.into(), &instance_id.into(), &owner)?;

				// transfer loan nft to loan pallet
				// ideally we should burn this but we do not have a function to burn them yet.
				// TODO(ved): burn loan nft when the functionality is available
				let (loan_class_id, loan_id) = loan_nft.destruct();
				T::NonFungible::transfer(
					&loan_class_id.into(),
					&loan_id.into(),
					&Self::account_id(),
				)?;

				// update loan status
				loan_info.status = LoanStatus::Closed;
				*maybe_loan_info = Some(loan_info);
				Ok(asset)
			},
		)
	}

	pub(crate) fn borrow_amount(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		owner: T::AccountId,
		amount: T::Amount,
	) -> DispatchResult {
		// ensure owner is the loan owner
		Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		LoanInfo::<T>::try_mutate(pool_id, loan_id, |maybe_loan_info| -> DispatchResult {
			let mut loan_info = maybe_loan_info.take().ok_or(Error::<T>::ErrMissingLoan)?;

			// ensure loan is active
			ensure!(
				loan_info.status == LoanStatus::Active,
				Error::<T>::ErrLoanNotActive
			);

			// ensure maturity date has not passed if the loan has a maturity date
			let now: u64 = Self::time_now()?;
			let valid = match loan_info.loan_type.maturity_date() {
				// loan has a maturity date
				Some(md) => md > now,
				// no maturity date, so continue as is
				None => true,
			};
			ensure!(valid, Error::<T>::ErrLoanMaturityDatePassed);

			// ensure borrow amount is positive
			ensure!(amount.is_positive(), Error::<T>::ErrLoanValueInvalid);

			// check for ceiling threshold
			ensure!(
				amount + loan_info.borrowed_amount <= loan_info.ceiling,
				Error::<T>::ErrLoanCeilingReached
			);

			// get previous present value so that we can update the nav accordingly
			let old_pv = loan_info
				.present_value()
				.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;

			// calculate accumulated rate and outstanding debt
			let (accumulated_rate, debt) = loan_info
				.accrue(now)
				.ok_or(Error::<T>::ErrLoanAccrueFailed)?;

			let new_borrowed_amount = loan_info
				.borrowed_amount
				.checked_add(&amount)
				.ok_or(Error::<T>::ErrAddAmountOverflow)?;

			// calculate new principal debt with adjustment amount
			let principal_debt = math::calculate_principal_debt::<T::Amount, T::Rate>(
				debt,
				math::Adjustment::Inc(amount),
				accumulated_rate,
			)
			.ok_or(Error::<T>::ErrPrincipalDebtOverflow)?;

			// update loan
			loan_info.borrowed_amount = new_borrowed_amount;
			loan_info.last_updated = now;
			loan_info.accumulated_rate = accumulated_rate;
			loan_info.principal_debt = principal_debt;
			let new_pv = loan_info
				.present_value()
				.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;
			Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;
			T::PoolReserve::transfer_to(
				pool_id,
				RawOrigin::Signed(Self::account_id()).into(),
				owner,
				amount.into(),
			)?;
			*maybe_loan_info = Some(loan_info);
			Ok(())
		})
	}

	pub(crate) fn update_nav_with_updated_present_value(
		pool_id: PoolIdOf<T>,
		new_pv: T::Amount,
		old_pv: T::Amount,
	) -> Result<(), DispatchError> {
		// calculate new diff from the old and new present value and update the nav accordingly
		PoolNAV::<T>::try_mutate(pool_id, |maybe_nav_details| -> Result<(), DispatchError> {
			let mut nav = maybe_nav_details.take().unwrap_or_default();
			let new_nav = match new_pv > old_pv {
				// borrow
				true => new_pv
					.checked_sub(&old_pv)
					.and_then(|positive_diff| nav.latest_nav.checked_add(&positive_diff)),
				// repay since new pv is less than old
				false => old_pv
					.checked_sub(&new_pv)
					.and_then(|negative_diff| nav.latest_nav.checked_sub(&negative_diff)),
			}
			.ok_or(Error::<T>::ErrAddAmountOverflow)?;
			nav.latest_nav = new_nav;
			*maybe_nav_details = Some(nav);
			Ok(())
		})
	}

	pub(crate) fn repay_amount(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		owner: T::AccountId,
		amount: T::Amount,
	) -> Result<T::Amount, DispatchError> {
		// ensure owner is the loan owner
		Self::check_loan_owner(pool_id, loan_id, owner.clone())?;

		LoanInfo::<T>::try_mutate(
			pool_id,
			loan_id,
			|maybe_loan_info| -> Result<T::Amount, DispatchError> {
				let mut loan_info = maybe_loan_info.take().ok_or(Error::<T>::ErrMissingLoan)?;

				// ensure loan is active
				ensure!(
					loan_info.status == LoanStatus::Active,
					Error::<T>::ErrLoanNotActive
				);

				// ensure repay amount is positive
				ensure!(amount.is_positive(), Error::<T>::ErrLoanValueInvalid);

				// calculate old present_value
				let old_pv = loan_info
					.present_value()
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;

				// calculate new accumulated rate
				let now: u64 = Self::time_now()?;
				let (accumulated_rate, debt) = loan_info
					.accrue(now)
					.ok_or(Error::<T>::ErrLoanAccrueFailed)?;

				// ensure amount is not more than current debt
				let mut repay_amount = amount;
				if repay_amount > debt {
					repay_amount = debt
				}

				// calculate new principal debt with repaid amount
				let principal_debt = math::calculate_principal_debt::<T::Amount, T::Rate>(
					debt,
					math::Adjustment::Dec(repay_amount),
					accumulated_rate,
				)
				.ok_or(Error::<T>::ErrAddAmountOverflow)?;

				loan_info.last_updated = now;
				loan_info.accumulated_rate = accumulated_rate;
				loan_info.principal_debt = principal_debt;
				let new_pv = loan_info
					.present_value()
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;
				Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;
				T::PoolReserve::transfer_from(
					pool_id,
					RawOrigin::Signed(Self::account_id()).into(),
					owner,
					repay_amount.into(),
				)?;
				*maybe_loan_info = Some(loan_info);
				Ok(repay_amount)
			},
		)
	}

	pub(crate) fn time_now() -> Result<u64, DispatchError> {
		let nowt = T::Time::now();
		TryInto::<u64>::try_into(nowt).map_err(|_| Error::<T>::ErrEpochTimeOverflow.into())
	}

	/// accrues rate and debt of a given loan and updates it
	/// returns the present value of the loan accounting any write offs
	pub(crate) fn accrue_and_update_loan(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		now: u64,
		write_off_groups: Vec<WriteOffGroup<T::Rate>>,
	) -> Result<T::Amount, DispatchError> {
		LoanInfo::<T>::try_mutate(
			pool_id,
			loan_id,
			|maybe_loan_data| -> Result<T::Amount, DispatchError> {
				let mut loan_data = maybe_loan_data.take().ok_or(Error::<T>::ErrMissingLoan)?;
				// if the loan is not active, then skip updating and return PV as zero
				if loan_data.status != LoanStatus::Active {
					*maybe_loan_data = Some(loan_data);
					return Ok(Zero::zero());
				}

				let (acc_rate, _debt) = loan_data
					.accrue(now)
					.ok_or(Error::<T>::ErrLoanAccrueFailed)?;
				loan_data.last_updated = now;
				loan_data.accumulated_rate = acc_rate;
				let present_value = loan_data
					.present_value_with_write_off(write_off_groups)
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;
				*maybe_loan_data = Some(loan_data);
				Ok(present_value)
			},
		)
	}

	/// updates nav for the given pool and returns the latest NAV at this instant
	pub(crate) fn update_nav_of_pool(pool_id: PoolIdOf<T>) -> Result<T::Amount, DispatchError> {
		let now = Self::time_now()?;
		let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
		let nav = LoanInfo::<T>::iter_key_prefix(pool_id).try_fold(
			Zero::zero(),
			|sum, loan_id| -> Result<T::Amount, DispatchError> {
				let pv =
					Self::accrue_and_update_loan(pool_id, loan_id, now, write_off_groups.clone())?;
				sum.checked_add(&pv)
					.ok_or(Error::<T>::ErrLoanAccrueFailed.into())
			},
		)?;
		PoolNAV::<T>::insert(
			pool_id,
			NAVDetails {
				latest_nav: nav,
				last_updated: now,
			},
		);
		Ok(nav)
	}

	pub(crate) fn add_write_off_group(
		pool_id: PoolIdOf<T>,
		group: WriteOffGroup<T::Rate>,
	) -> Result<u32, DispatchError> {
		// ensure pool is initialised
		ensure!(
			PoolToLoanNftClass::<T>::contains_key(pool_id),
			Error::<T>::ErrPoolNotInitialised,
		);

		// ensure write off percentage is not more than 100%
		ensure!(
			group.percentage <= One::one(),
			Error::<T>::ErrInvalidWriteOffGroup
		);

		// append new group
		let index = PoolWriteOffGroups::<T>::mutate(pool_id, |write_off_groups| -> u32 {
			write_off_groups.push(group);
			// return the index of the write off group
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
	pub(crate) fn write_off(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		override_write_off_index: Option<u32>,
	) -> Result<u32, DispatchError> {
		LoanInfo::<T>::try_mutate(
			pool_id,
			loan_id,
			|maybe_loan_data| -> Result<u32, DispatchError> {
				let mut loan_data = maybe_loan_data.take().ok_or(Error::<T>::ErrMissingLoan)?;
				// ensure loan is active
				ensure!(
					loan_data.status == LoanStatus::Active,
					Error::<T>::ErrLoanNotActive
				);

				let now = Self::time_now()?;

				// ensure loan was not overwritten by admin and try to fetch a valid write off group for loan
				let write_off_groups = PoolWriteOffGroups::<T>::get(pool_id);
				let write_off_group_index = match override_write_off_index {
					// admin is trying to write off
					Some(index) => {
						// check if the write off group exists
						write_off_groups
							.get(index as usize)
							.ok_or(Error::<T>::ErrInvalidWriteOffGroupIndex)?;
						loan_data.admin_written_off = true;
						Ok(index)
					}
					None => {
						// non-admin is trying to write off but admin already did. So error out
						if loan_data.admin_written_off {
							return Err(Error::<T>::ErrLoanWrittenOffByAdmin.into());
						}

						let maturity_date = loan_data
							.loan_type
							.maturity_date()
							.ok_or(Error::<T>::ErrLoanTypeInvalid)?;

						// ensure loan's maturity date has passed
						ensure!(now > maturity_date, Error::<T>::ErrLoanHealthy);

						// not written off by admin, and non admin trying to write off, then
						// fetch the best write group available for this loan
						math::valid_write_off_group(maturity_date, now, write_off_groups.clone())
							.ok_or(Error::<T>::ErrNoValidWriteOffGroup)
					}
				}?;

				// get old present value accounting for any write offs
				let old_pv = loan_data
					.present_value_with_write_off(write_off_groups.clone())
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;

				// accrue and calculate the new present value with current chosen write off
				let (accumulated_rate, _current_debt) = loan_data
					.accrue(now)
					.ok_or(Error::<T>::ErrLoanAccrueFailed)?;

				loan_data.accumulated_rate = accumulated_rate;
				loan_data.last_updated = now;
				loan_data.write_off_index = Some(write_off_group_index);

				// calculate updated write off adjusted present value
				let new_pv = loan_data
					.present_value_with_write_off(write_off_groups)
					.ok_or(Error::<T>::ErrLoanPresentValueFailed)?;

				// update nav
				Self::update_nav_with_updated_present_value(pool_id, new_pv, old_pv)?;

				// update loan data
				*maybe_loan_data = Some(loan_data);
				Ok(write_off_group_index)
			},
		)
	}
}
