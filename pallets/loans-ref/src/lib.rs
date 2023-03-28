// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

#![cfg_attr(not(feature = "std"), no_std)]

//! This pallet offers extrinsics to handle loans.
//!
//! The following actions are performed over loans:
//!
//! | Extrinsics                    | Role      |
//! |-------------------------------|-----------|
//! | [`Pallet::create()`]          | Borrower  |
//! | [`Pallet::borrow()`]          | Borrower  |
//! | [`Pallet::repay()`]           | Borrower  |
//! | [`Pallet::write_off()`]       | Any       |
//! | [`Pallet::admin_write_off()`] | LoanAdmin |
//! | [`Pallet::close()`]           | Borrower  |
//!
//! The following actions are performed over a pool of loans:
//!
//! | Extrinsics                               | Role      |
//! |------------------------------------------|-----------|
//! | [`Pallet::update_write_off_policy()`]    | PoolAdmin |
//! | [`Pallet::update_portfolio_valuation()`] | Any       |
//!
//! The whole pallet is optimized for the more expensive extrinsic that is
//! [`Pallet::update_portfolio_valuation()`] that should go through all active loans.

pub mod types;
pub mod valuation;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use cfg_primitives::Moment;
	use cfg_traits::{
		ops::{EnsureAdd, EnsureAddAssign, EnsureInto},
		InterestAccrual, Permissions, PoolInspect, PoolNAV, PoolReserve,
	};
	use cfg_types::{
		adjustments::Adjustment,
		permissions::{PermissionScope, PoolRole, Role},
	};
	use frame_support::{
		pallet_prelude::*,
		traits::{
			tokens::{
				self,
				nonfungibles::{Inspect, Transfer},
			},
			UnixTime,
		},
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_arithmetic::FixedPointNumber;
	use sp_runtime::{
		traits::{BadOrigin, One, Zero},
		ArithmeticError, FixedPointOperand,
	};
	use types::{
		self, ActiveLoan, AssetOf, BorrowLoanError, CloseLoanError, CreateLoanError, LoanInfoOf,
		PortfolioValuationUpdateType, WriteOffState, WriteOffStatus, WrittenOffError,
	};

	use super::*;

	pub type PoolIdOf<T> = <<T as Config>::Pool as PoolInspect<
		<T as frame_system::Config>::AccountId,
		<T as Config>::CurrencyId,
	>>::PoolId;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Identify a curreny.
		type CurrencyId: Parameter + Copy + MaxEncodedLen;

		/// Identify a non fungible collection
		type CollectionId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Default
			+ TypeInfo
			+ Copy
			+ MaxEncodedLen;

		/// Identify a non fungible item
		type ItemId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Default
			+ TypeInfo
			+ Copy
			+ MaxEncodedLen;

		/// Identify a loan in the pallet
		type LoanId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Default
			+ TypeInfo
			+ MaxEncodedLen
			+ Copy
			+ EnsureAdd
			+ One;

		/// Defines the rate type used for math computations
		type Rate: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ TypeInfo
			+ MaxEncodedLen;

		/// Defines the balance type used for math computations
		type Balance: tokens::Balance + FixedPointOperand;

		/// Fetching method for the time of the current block
		type Time: UnixTime;

		/// Used to mint, transfer, and inspect assets.
		type NonFungible: Transfer<Self::AccountId>
			+ Inspect<Self::AccountId, CollectionId = Self::CollectionId, ItemId = Self::ItemId>;

		/// Access to the pool
		type Pool: PoolReserve<Self::AccountId, Self::CurrencyId, Balance = Self::Balance>;

		/// Used to verify permissions of users
		type Permissions: Permissions<
			Self::AccountId,
			Scope = PermissionScope<PoolIdOf<Self>, Self::CurrencyId>,
			Role = Role,
			Error = DispatchError,
		>;

		/// Used to calculate interest accrual for debt.
		type InterestAccrual: InterestAccrual<
			Self::Rate,
			Self::Balance,
			Adjustment<Self::Balance>,
			NormalizedDebt = Self::Balance,
		>;

		/// Max number of active loans per pool.
		#[pallet::constant]
		type MaxActiveLoansPerPool: Get<u32>;

		/// Max number of write-off groups per pool.
		#[pallet::constant]
		type MaxWriteOffPolicySize: Get<u32>;

		/// Information of runtime weights
		type WeightInfo: WeightInfo;
	}

	/// Contains the last loan id generated
	#[pallet::storage]
	pub(crate) type LastLoanId<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolIdOf<T>, T::LoanId, ValueQuery>;

	/// Storage for loans that has been created but are not still active.
	#[pallet::storage]
	pub(crate) type CreatedLoan<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Blake2_128Concat,
		T::LoanId,
		types::CreatedLoan<T>,
		OptionQuery,
	>;

	/// Storage for active loans.
	/// The indexation of this storage differs from `CreatedLoan` or `ClosedLoan`
	/// because here we try to minimize the iteration speed over all active loans in a pool.
	/// `Moment` value along with the `ActiveLoan` correspond to the last moment the active loan was
	/// used to compute the portfolio valuation in an inexact way.
	#[pallet::storage]
	pub(crate) type ActiveLoans<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		BoundedVec<(ActiveLoan<T>, Moment), T::MaxActiveLoansPerPool>,
		ValueQuery,
	>;

	/// Storage for closed loans.
	/// No mutations are expected in this storage.
	/// Loans are stored here for historical purposes.
	#[pallet::storage]
	pub(crate) type ClosedLoan<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Blake2_128Concat,
		T::LoanId,
		types::ClosedLoan<T>,
		OptionQuery,
	>;

	/// Stores write off policy used in each pool
	#[pallet::storage]
	pub(crate) type WriteOffPolicy<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		BoundedVec<WriteOffState<T::Rate>, T::MaxWriteOffPolicySize>,
		ValueQuery,
	>;

	/// Stores the portfolio valuation associated to each pool
	#[pallet::storage]
	#[pallet::getter(fn portfolio_valuation)]
	pub(crate) type PortfolioValuation<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		types::PortfolioValuation<T::Balance>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A loan was created
		Created {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			loan_info: LoanInfoOf<T>,
		},
		/// An amount was borrowed for a loan
		Borrowed {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		},
		/// An amount was repaid for a loan
		Repaid {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		},
		/// A loan was written off
		WrittenOff {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			status: WriteOffStatus<T::Rate>,
		},
		/// A loan was closed
		Closed {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			collateral: AssetOf<T>,
		},
		/// The Portfolio Valuation for a pool was updated.
		PortfolioValuationUpdated {
			pool_id: PoolIdOf<T>,
			valuation: T::Balance,
			update_type: PortfolioValuationUpdateType,
		},
		WriteOffPolicyUpdated {
			pool_id: PoolIdOf<T>,
			policy: BoundedVec<WriteOffState<T::Rate>, T::MaxWriteOffPolicySize>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when pool doesn't exist
		PoolNotFound,
		/// Emits when loan doesn't exist
		LoanNotFound,
		/// Emits when a loan exist but it's not active
		LoanNotActive,
		/// Emits when a write-off state is not found in a policy for a specific loan.
		/// It happens when there is no policy or the loan is not overdue.
		NoValidWriteOffState,
		/// Emits when the NFT owner is not found
		NFTOwnerNotFound,
		/// Emits when NFT owner doesn't match the expected owner
		NotNFTOwner,
		/// Emits when the applicant account is not the borrower of the loan
		NotLoanBorrower,
		/// Emits when the max number of active loans was reached
		MaxActiveLoansReached,
		/// Emits when the loan is incorrectly specified and can not be created
		CreateLoanError(CreateLoanError),
		/// Emits when the loan can not be borrowed from
		BorrowLoanError(BorrowLoanError),
		/// Emits when the loan can not be written off
		WrittenOffError(WrittenOffError),
		/// Emits when the loan can not be closed
		CloseLoanError(CloseLoanError),
	}

	impl<T> From<CreateLoanError> for Error<T> {
		fn from(error: CreateLoanError) -> Self {
			Error::<T>::CreateLoanError(error)
		}
	}

	impl<T> From<BorrowLoanError> for Error<T> {
		fn from(error: BorrowLoanError) -> Self {
			Error::<T>::BorrowLoanError(error)
		}
	}

	impl<T> From<WrittenOffError> for Error<T> {
		fn from(error: WrittenOffError) -> Self {
			Error::<T>::WrittenOffError(error)
		}
	}

	impl<T> From<CloseLoanError> for Error<T> {
		fn from(error: CloseLoanError) -> Self {
			Error::<T>::CloseLoanError(error)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Creates a new loan against the collateral provided
		///
		/// The origin must be the owner of the collateral.
		/// This collateral will be transferred to the existing pool.
		#[pallet::weight(T::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			info: LoanInfoOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::Borrower)?;
			Self::ensure_collateral_owner(&who, *info.collateral())?;
			Self::ensure_pool_exists(pool_id)?;

			info.validate::<T>(Self::now())?;

			let collateral = info.collateral();
			T::NonFungible::transfer(&collateral.0, &collateral.1, &T::Pool::account_for(pool_id))?;

			let loan_id = Self::generate_loan_id(pool_id)?;
			CreatedLoan::<T>::insert(pool_id, loan_id, types::CreatedLoan::new(info.clone(), who));

			Self::deposit_event(Event::<T>::Created {
				pool_id,
				loan_id,
				loan_info: info,
			});

			Ok(())
		}

		/// Transfers borrow amount to the borrower.
		///
		/// The origin must be the borrower of the loan.
		/// The borrow action should fulfill the borrow restrictions configured at [`types::LoanRestrictions`].
		/// The `amount` will be transferred from pool reserve to borrower.
		/// The portfolio valuation of the pool is updated to reflect the new present value of the loan.
		/// Rate accumulation will start after the first borrow.
		#[pallet::weight(T::WeightInfo::borrow(T::MaxActiveLoansPerPool::get()))]
		pub fn borrow(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let _count = match CreatedLoan::<T>::take(pool_id, loan_id) {
				Some(created_loan) => {
					Self::ensure_loan_borrower(&who, created_loan.borrower())?;

					let mut active_loan = created_loan.activate(loan_id)?;
					active_loan.borrow(amount)?;

					Self::insert_active_loan(pool_id, active_loan)?
				}
				None => {
					Self::update_active_loan(pool_id, loan_id, |loan| {
						Self::ensure_loan_borrower(&who, loan.borrower())?;
						loan.borrow(amount)
					})?
					.1
				}
			};

			T::Pool::withdraw(pool_id, who, amount)?;

			Self::deposit_event(Event::<T>::Borrowed {
				pool_id,
				loan_id,
				amount,
			});

			Ok(())
		}

		/// Transfers amount borrowed to the pool reserve.
		///
		/// The origin must be the borrower of the loan.
		/// If the repaying amount is more than current debt, only current debt is transferred.
		/// The borrow action should fulfill the borrow restrictions configured at [`types::LoanRestrictions`].
		/// The `amount` will be transferred from borrower to pool reserve.
		/// The portfolio valuation of the pool is updated to reflect the new present value of the loan.
		#[pallet::weight(T::WeightInfo::repay(T::MaxActiveLoansPerPool::get()))]
		pub fn repay(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let (amount, _count) = Self::update_active_loan(pool_id, loan_id, |loan| {
				Self::ensure_loan_borrower(&who, loan.borrower())?;
				loan.repay(amount)
			})?;

			T::Pool::deposit(pool_id, who, amount)?;

			Self::deposit_event(Event::<T>::Repaid {
				pool_id,
				loan_id,
				amount,
			});

			Ok(())
		}

		/// Writes off an overdue loan.
		///
		/// This action will write off based on the write off policy configured by
		/// [`Pallet::update_write_off_policy()`].
		/// The write off action will only take effect if it writes down more (percentage or penalty)
		/// than the current write off status of the loan. This action will never writes up. i.e:
		/// - Write off by admin with percentage 0.5 and penalty 0.2
		/// - Time passes and the policy can be applied.
		/// - Write of with a policy that says: percentage 0.3, penaly 0.4
		/// - The loan is written off with the maximum between the policy and the current state:
		///   percentage 0.5, penaly 0.4
		///
		/// No special permisions are required to this call.
		/// The portfolio valuation of the pool is updated to reflect the new present value of the loan.
		#[pallet::weight(T::WeightInfo::write_off(T::MaxActiveLoansPerPool::get()))]
		pub fn write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let (status, _count) = Self::update_active_loan(pool_id, loan_id, |loan| {
				let state = Self::find_write_off_state(pool_id, loan.maturity_date())?;
				let limit = state.status().max(loan.write_off_status());

				loan.write_off(&limit, &limit)?;

				Ok(limit)
			})?;

			Self::deposit_event(Event::<T>::WrittenOff {
				pool_id,
				loan_id,
				status,
			});

			Ok(())
		}

		/// Writes off a loan from admin origin.
		///
		/// Forces a writing off of a loan if the `percentage` and `penalty` parameters
		/// respecting the policy values as the minimum.
		/// This action can write down/up the current write off status of the loan.
		/// If there is no active policy, an admin write off action can write up the write off
		/// status. But if there is a policy applied, the admin can only write up until the policy.
		/// Write down more than the policy is always allowed.
		/// The portfolio valuation of the pool is updated to reflect the new present value of the loan.
		#[pallet::weight(T::WeightInfo::admin_write_off(T::MaxActiveLoansPerPool::get()))]
		pub fn admin_write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			percentage: T::Rate,
			penalty: T::Rate,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::LoanAdmin)?;

			let status = WriteOffStatus {
				percentage,
				penalty: Self::to_rate_per_sec(penalty)?,
			};

			let _count = Self::update_active_loan(pool_id, loan_id, |loan| {
				let state = Self::find_write_off_state(pool_id, loan.maturity_date());
				let limit = state.map(|s| s.status()).unwrap_or_else(|_| status.clone());

				loan.write_off(&limit, &status)
			})?;

			Self::deposit_event(Event::<T>::WrittenOff {
				pool_id,
				loan_id,
				status,
			});

			Ok(())
		}

		/// Closes a given loan
		///
		/// A loan only can be closed if it's fully repaid by the loan borrower.
		/// Closing a loan gives back the collateral used for the loan to the borrower .
		#[pallet::weight(T::WeightInfo::close(T::MaxActiveLoansPerPool::get()))]
		pub fn close(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let ((closed_loan, borrower), _count) = match CreatedLoan::<T>::take(pool_id, loan_id) {
				Some(created_loan) => (created_loan.close()?, Zero::zero()),
				None => {
					let (active_loan, count) = Self::take_active_loan(pool_id, loan_id)?;
					(active_loan.close()?, count)
				}
			};

			Self::ensure_loan_borrower(&who, &borrower)?;

			let collateral = closed_loan.collateral();
			T::NonFungible::transfer(&collateral.0, &collateral.1, &who)?;

			ClosedLoan::<T>::insert(pool_id, loan_id, closed_loan);

			Self::deposit_event(Event::<T>::Closed {
				pool_id,
				loan_id,
				collateral,
			});

			Ok(())
		}

		/// Updates the write off policy.
		///
		/// The write off policy is used to automatically set a write off minimum value to the
		/// loan.
		#[pallet::weight(T::WeightInfo::update_write_off_policy())]
		pub fn update_write_off_policy(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			mut policy: BoundedVec<WriteOffState<T::Rate>, T::MaxWriteOffPolicySize>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::PoolAdmin)?;
			Self::ensure_pool_exists(pool_id)?;

			policy.iter_mut().try_for_each(|state| -> DispatchResult {
				state.penalty = Self::to_rate_per_sec(state.penalty)?;
				Ok(())
			})?;

			WriteOffPolicy::<T>::insert(pool_id, policy.clone());

			Self::deposit_event(Event::<T>::WriteOffPolicyUpdated { pool_id, policy });

			Ok(())
		}

		/// Updates the porfolio valuation for the given pool
		#[pallet::weight(T::WeightInfo::update_portfolio_valuation(
			T::MaxActiveLoansPerPool::get()
		))]
		pub fn update_portfolio_valuation(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;
			Self::ensure_pool_exists(pool_id)?;

			let (value, count) = Self::portfolio_valuation_for_pool(pool_id)?;

			PortfolioValuation::<T>::insert(
				pool_id,
				types::PortfolioValuation::new(value, Self::now()),
			);

			Self::deposit_event(Event::<T>::PortfolioValuationUpdated {
				pool_id,
				valuation: value,
				update_type: PortfolioValuationUpdateType::Exact,
			});

			Ok(Some(T::WeightInfo::update_portfolio_valuation(count)).into())
		}
	}

	/// Utility methods
	impl<T: Config> Pallet<T> {
		fn now() -> Moment {
			T::Time::now().as_secs()
		}

		fn ensure_role(pool_id: PoolIdOf<T>, who: &T::AccountId, role: PoolRole) -> DispatchResult {
			T::Permissions::has(
				PermissionScope::Pool(pool_id),
				who.clone(),
				Role::PoolRole(role),
			)
			.then_some(())
			.ok_or_else(|| BadOrigin.into())
		}

		fn ensure_collateral_owner(
			owner: &T::AccountId,
			(collection_id, item_id): AssetOf<T>,
		) -> DispatchResult {
			T::NonFungible::owner(&collection_id, &item_id)
				.ok_or(Error::<T>::NFTOwnerNotFound)?
				.eq(owner)
				.then_some(())
				.ok_or_else(|| Error::<T>::NotNFTOwner.into())
		}

		fn ensure_loan_borrower(owner: &T::AccountId, borrower: &T::AccountId) -> DispatchResult {
			ensure!(owner == borrower, Error::<T>::NotLoanBorrower);
			Ok(())
		}

		fn ensure_pool_exists(pool_id: PoolIdOf<T>) -> DispatchResult {
			ensure!(T::Pool::pool_exists(pool_id), Error::<T>::PoolNotFound);
			Ok(())
		}

		fn generate_loan_id(pool_id: PoolIdOf<T>) -> Result<T::LoanId, ArithmeticError> {
			LastLoanId::<T>::try_mutate(pool_id, |last_loan_id| {
				last_loan_id.ensure_add_assign(One::one())?;
				Ok(*last_loan_id)
			})
		}

		fn to_rate_per_sec(rate_per_year: T::Rate) -> Result<T::Rate, DispatchError> {
			T::InterestAccrual::convert_additive_rate_to_per_sec(rate_per_year)
		}

		fn find_write_off_state(
			pool_id: PoolIdOf<T>,
			maturity_date: Moment,
		) -> Result<WriteOffState<T::Rate>, DispatchError> {
			WriteOffState::find_best(
				WriteOffPolicy::<T>::get(pool_id).into_iter(),
				maturity_date,
				T::Time::now().as_secs(),
			)
			.ok_or_else(|| Error::<T>::NoValidWriteOffState.into())
		}

		fn update_portfolio_valuation_with_pv(
			pool_id: PoolIdOf<T>,
			portfolio: &mut types::PortfolioValuation<T::Balance>,
			old_pv: T::Balance,
			new_pv: T::Balance,
		) -> DispatchResult {
			let prev_value = portfolio.value();

			portfolio.update_with_pv_diff(old_pv, new_pv)?;

			if prev_value != portfolio.value() {
				Self::deposit_event(Event::<T>::PortfolioValuationUpdated {
					pool_id,
					valuation: portfolio.value(),
					update_type: PortfolioValuationUpdateType::Inexact,
				});
			}

			Ok(())
		}

		fn portfolio_valuation_for_pool(
			pool_id: PoolIdOf<T>,
		) -> Result<(T::Balance, u32), DispatchError> {
			let rates = T::InterestAccrual::rates();
			let loans = ActiveLoans::<T>::get(pool_id);
			let count = loans.len().ensure_into()?;
			let value = loans.into_iter().try_fold(
				T::Balance::zero(),
				|sum, (loan, _)| -> Result<T::Balance, DispatchError> {
					Ok(sum.ensure_add(loan.current_present_value(&rates)?)?)
				},
			)?;

			Ok((value, count))
		}

		fn insert_active_loan(
			pool_id: PoolIdOf<T>,
			loan: ActiveLoan<T>,
		) -> Result<u32, DispatchError> {
			PortfolioValuation::<T>::try_mutate(pool_id, |portfolio| {
				let last_updated = Self::now();
				let new_pv = loan.present_value_at(last_updated)?;

				Self::update_portfolio_valuation_with_pv(pool_id, portfolio, Zero::zero(), new_pv)?;

				ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
					active_loans
						.try_push((loan, last_updated))
						.map_err(|_| Error::<T>::MaxActiveLoansReached)?;

					Ok(active_loans.len().ensure_into()?)
				})
			})
		}

		fn update_active_loan<F, R>(
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			f: F,
		) -> Result<(R, u32), DispatchError>
		where
			F: FnOnce(&mut ActiveLoan<T>) -> Result<R, DispatchError>,
		{
			PortfolioValuation::<T>::try_mutate(pool_id, |portfolio| {
				ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
					let (loan, last_updated) = active_loans
						.iter_mut()
						.find(|(loan, _)| loan.loan_id() == loan_id)
						.ok_or_else(|| {
							if CreatedLoan::<T>::contains_key(pool_id, loan_id) {
								Error::<T>::LoanNotActive
							} else {
								Error::<T>::LoanNotFound
							}
						})?;

					*last_updated = (*last_updated).max(portfolio.last_updated());
					let old_pv = loan.present_value_at(*last_updated)?;

					let result = f(loan)?;

					*last_updated = Self::now();
					let new_pv = loan.present_value_at(*last_updated)?;

					Self::update_portfolio_valuation_with_pv(pool_id, portfolio, old_pv, new_pv)?;

					Ok((result, active_loans.len().ensure_into()?))
				})
			})
		}

		fn take_active_loan(
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> Result<(ActiveLoan<T>, u32), DispatchError> {
			ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
				let index = active_loans
					.iter()
					.position(|(loan, _)| loan.loan_id() == loan_id)
					.ok_or(Error::<T>::LoanNotFound)?;

				Ok((
					active_loans.swap_remove(index).0,
					active_loans.len().ensure_into()?,
				))
			})
		}

		#[cfg(feature = "runtime-benchmarks")]
		/// Set the maturity date of the loan to this instant.
		pub fn expire(pool_id: PoolIdOf<T>, loan_id: T::LoanId) -> DispatchResult {
			Self::update_active_loan(pool_id, loan_id, |loan| {
				loan.set_maturity(T::Time::now());
				Ok(())
			})?;
			Ok(())
		}
	}

	// TODO: This implementation can be cleaned once #908 be solved
	impl<T: Config> PoolNAV<PoolIdOf<T>, T::Balance> for Pallet<T> {
		type ClassId = T::ItemId;
		type RuntimeOrigin = T::RuntimeOrigin;

		fn nav(pool_id: PoolIdOf<T>) -> Option<(T::Balance, Moment)> {
			let valuation = PortfolioValuation::<T>::get(pool_id);
			Some((valuation.value(), valuation.last_updated()))
		}

		fn update_nav(pool_id: PoolIdOf<T>) -> Result<T::Balance, DispatchError> {
			let (value, _) = Self::portfolio_valuation_for_pool(pool_id)?;

			PortfolioValuation::<T>::insert(
				pool_id,
				types::PortfolioValuation::new(value, Self::now()),
			);

			Ok(value)
		}

		fn initialise(_: OriginFor<T>, _: PoolIdOf<T>, _: T::ItemId) -> DispatchResult {
			// This Loans implementation does not need to initialize explicitally.
			Ok(())
		}
	}
}
