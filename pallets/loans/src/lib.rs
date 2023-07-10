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
//! The following actions are performed over a loan:
//!
//! | Extrinsics                          | Role      |
//! |-------------------------------------|-----------|
//! | [`Pallet::create()`]                | Borrower  |
//! | [`Pallet::borrow()`]                | Borrower  |
//! | [`Pallet::repay()`]                 | Borrower  |
//! | [`Pallet::write_off()`]             |           |
//! | [`Pallet::admin_write_off()`]       | LoanAdmin |
//! | [`Pallet::propose_loan_mutation()`] | LoanAdmin |
//! | [`Pallet::apply_loan_mutation()`]   |           |
//! | [`Pallet::close()`]                 | Borrower  |
//!
//! The following actions are performed over an entire pool of loans:
//!
//! | Extrinsics                               | Role      |
//! |------------------------------------------|-----------|
//! | [`Pallet::propose_write_off_policy()`]   | PoolAdmin |
//! | [`Pallet::apply_write_off_policy()`]     |           |
//! | [`Pallet::update_portfolio_valuation()`] |           |
//!
//! The whole pallet is optimized for the more expensive extrinsic that is
//! [`Pallet::update_portfolio_valuation()`] that should go through all active
//! loans.

pub mod migrations {
	pub mod nuke;
}

/// High level types that uses `pallet::Config`
pub mod entities {
	pub mod balance;
	pub mod interest;
	pub mod loans;
	pub mod pricing;
}

/// Low level types that doesn't know about what a pallet is
pub mod types;

/// Utility types for configure the pallet from a runtime
pub mod util;

mod weights;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use cfg_primitives::Moment;
	use cfg_traits::{
		self,
		changes::ChangeGuard,
		data::{DataCollection, DataRegistry},
		InterestAccrual, Permissions, PoolInspect, PoolNAV, PoolReserve,
	};
	use cfg_types::{
		adjustments::Adjustment,
		permissions::{PermissionScope, PoolRole, Role},
	};
	use codec::HasCompact;
	use entities::loans::{self, ActiveLoan, LoanInfo};
	use frame_support::{
		pallet_prelude::*,
		storage::transactional,
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
		traits::{BadOrigin, EnsureAdd, EnsureAddAssign, EnsureInto, One, Zero},
		ArithmeticError, FixedPointOperand, TransactionOutcome,
	};
	use sp_std::vec::Vec;
	use types::{
		self,
		policy::{self, WriteOffRule, WriteOffStatus},
		portfolio::{self, InitialPortfolioValuation, PortfolioValuationUpdateType},
		BorrowLoanError, Change, CloseLoanError, CreateLoanError, LoanMutation, MutationError,
		RepaidAmount, RepayLoanError, WrittenOffError,
	};

	use super::*;

	pub type PriceCollectionOf<T> = <<T as Config>::PriceRegistry as DataRegistry<
		<T as Config>::PriceId,
		<T as Config>::PoolId,
	>>::Collection;

	pub type AssetOf<T> = (<T as Config>::CollectionId, <T as Config>::ItemId);
	pub type PriceOf<T> = (<T as Config>::Rate, Moment);
	pub type PriceResultOf<T> = Result<PriceOf<T>, DispatchError>;
	pub type ChangeOf<T> =
		Change<<T as Config>::LoanId, <T as Config>::Rate, <T as Config>::MaxWriteOffPolicySize>;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Represent a runtime change
		type RuntimeChange: From<ChangeOf<Self>> + TryInto<ChangeOf<Self>>;

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

		/// Identify a loan in the pallet
		type PriceId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ Copy
			+ MaxEncodedLen;

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

		/// The PoolId type
		type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;

		/// Access to the pool
		type Pool: PoolReserve<
			Self::AccountId,
			Self::CurrencyId,
			Balance = Self::Balance,
			PoolId = Self::PoolId,
		>;

		/// Used to verify permissions of users
		type Permissions: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role,
			Error = DispatchError,
		>;

		/// Used to fetch and update Oracle prices
		type PriceRegistry: DataRegistry<Self::PriceId, Self::PoolId, Data = PriceResultOf<Self>>;

		/// Used to calculate interest accrual for debt.
		type InterestAccrual: InterestAccrual<
			Self::Rate,
			Self::Balance,
			Adjustment<Self::Balance>,
			NormalizedDebt = Self::Balance,
		>;

		/// Used to notify the runtime about changes that require special
		/// treatment.
		type ChangeGuard: ChangeGuard<
			PoolId = Self::PoolId,
			ChangeId = Self::Hash,
			Change = Self::RuntimeChange,
		>;

		/// Max number of active loans per pool.
		#[pallet::constant]
		type MaxActiveLoansPerPool: Get<u32>;

		/// Max number of write-off groups per pool.
		#[pallet::constant]
		type MaxWriteOffPolicySize: Get<u32> + Parameter;

		/// Information of runtime weights
		type WeightInfo: WeightInfo;
	}

	/// Contains the last loan id generated
	#[pallet::storage]
	pub(crate) type LastLoanId<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, T::LoanId, ValueQuery>;

	/// Storage for loans that has been created but are not still active.
	#[pallet::storage]
	pub(crate) type CreatedLoan<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::LoanId,
		loans::CreatedLoan<T>,
		OptionQuery,
	>;

	/// Storage for active loans.
	/// The indexation of this storage differs from `CreatedLoan` or
	/// `ClosedLoan` because here we try to minimize the iteration speed over
	/// all active loans in a pool. `Moment` value along with the `ActiveLoan`
	/// correspond to the last moment the active loan was used to compute the
	/// portfolio valuation in an inexact way.
	#[pallet::storage]
	pub(crate) type ActiveLoans<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		BoundedVec<(T::LoanId, ActiveLoan<T>), T::MaxActiveLoansPerPool>,
		ValueQuery,
	>;

	/// Storage for closed loans.
	/// No mutations are expected in this storage.
	/// Loans are stored here for historical purposes.
	#[pallet::storage]
	pub(crate) type ClosedLoan<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::LoanId,
		loans::ClosedLoan<T>,
		OptionQuery,
	>;

	/// Stores write off policy used in each pool
	#[pallet::storage]
	pub(crate) type WriteOffPolicy<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		BoundedVec<WriteOffRule<T::Rate>, T::MaxWriteOffPolicySize>,
		ValueQuery,
	>;

	/// Stores the portfolio valuation associated to each pool
	#[pallet::storage]
	#[pallet::getter(fn portfolio_valuation)]
	pub(crate) type PortfolioValuation<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		portfolio::PortfolioValuation<T::Balance, T::LoanId, T::MaxActiveLoansPerPool>,
		ValueQuery,
		InitialPortfolioValuation<T::Time>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A loan was created
		Created {
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			loan_info: LoanInfo<T>,
		},
		/// An amount was borrowed for a loan
		Borrowed {
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			amount: T::Balance,
		},
		/// An amount was repaid for a loan
		Repaid {
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			amount: RepaidAmount<T::Balance>,
		},
		/// A loan was written off
		WrittenOff {
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			status: WriteOffStatus<T::Rate>,
		},
		/// An active loan was mutated
		Mutated {
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			mutation: LoanMutation<T::Rate>,
		},
		/// A loan was closed
		Closed {
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			collateral: AssetOf<T>,
		},
		/// The Portfolio Valuation for a pool was updated.
		PortfolioValuationUpdated {
			pool_id: T::PoolId,
			valuation: T::Balance,
			update_type: PortfolioValuationUpdateType,
		},
		WriteOffPolicyUpdated {
			pool_id: T::PoolId,
			policy: BoundedVec<WriteOffRule<T::Rate>, T::MaxWriteOffPolicySize>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when pool doesn't exist
		PoolNotFound,
		/// Emits when loan doesn't exist or it's not active yet.
		LoanNotActiveOrNotFound,
		/// Emits when a write-off rule is not found in a policy for a specific
		/// loan. It happens when there is no policy or the loan is not overdue.
		NoValidWriteOffRule,
		/// Emits when the NFT owner is not found
		NFTOwnerNotFound,
		/// Emits when NFT owner doesn't match the expected owner
		NotNFTOwner,
		/// Emits when the applicant account is not the borrower of the loan
		NotLoanBorrower,
		/// Emits when the max number of active loans was reached
		MaxActiveLoansReached,
		/// Emits when an amount used is not multiple of the current price
		AmountNotMultipleOfPrice,
		/// The Change Id does not belong to a loan change
		NoLoanChangeId,
		/// The Change Id exists but it's not releated with the expected change
		UnrelatedChangeId,
		/// Emits when the loan is incorrectly specified and can not be created
		CreateLoanError(CreateLoanError),
		/// Emits when the loan can not be borrowed from
		BorrowLoanError(BorrowLoanError),
		/// Emits when the loan can not be repaid from
		RepayLoanError(RepayLoanError),
		/// Emits when the loan can not be written off
		WrittenOffError(WrittenOffError),
		/// Emits when the loan can not be closed
		CloseLoanError(CloseLoanError),
		/// Emits when the loan can not be mutated
		MutationError(MutationError),
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

	impl<T> From<RepayLoanError> for Error<T> {
		fn from(error: RepayLoanError) -> Self {
			Error::<T>::RepayLoanError(error)
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

	impl<T> From<MutationError> for Error<T> {
		fn from(error: MutationError) -> Self {
			Error::<T>::MutationError(error)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		PriceCollectionOf<T>: DataCollection<T::PriceId, Data = PriceResultOf<T>>,
	{
		/// Creates a new loan against the collateral provided
		///
		/// The origin must be the owner of the collateral.
		/// This collateral will be transferred to the existing pool.
		#[pallet::weight(T::WeightInfo::create())]
		#[pallet::call_index(0)]
		pub fn create(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			info: LoanInfo<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::Borrower)?;
			Self::ensure_collateral_owner(&who, info.collateral())?;
			Self::ensure_pool_exists(pool_id)?;

			info.validate(Self::now())?;

			let collateral = info.collateral();
			T::NonFungible::transfer(&collateral.0, &collateral.1, &T::Pool::account_for(pool_id))?;

			let loan_id = Self::generate_loan_id(pool_id)?;
			CreatedLoan::<T>::insert(pool_id, loan_id, loans::CreatedLoan::new(info.clone(), who));

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
		/// The borrow action should fulfill the borrow restrictions configured
		/// at [`types::LoanRestrictions`]. The `amount` will be transferred
		/// from pool reserve to borrower. The portfolio valuation of the pool
		/// is updated to reflect the new present value of the loan.
		/// Rate accumulation will start after the first borrow.
		#[pallet::weight(T::WeightInfo::borrow(T::MaxActiveLoansPerPool::get()))]
		#[pallet::call_index(1)]
		pub fn borrow(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let _count = match CreatedLoan::<T>::take(pool_id, loan_id) {
				Some(created_loan) => {
					Self::ensure_loan_borrower(&who, created_loan.borrower())?;

					let mut active_loan = created_loan.activate(pool_id)?;
					active_loan.borrow(amount)?;

					Self::insert_active_loan(pool_id, loan_id, active_loan)?
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
		/// The repay action should fulfill the repay restrictions
		/// configured at [`types::RepayRestrictions`].
		/// If the repaying `amount` is more than current debt, only current
		/// debt is transferred. This does not apply to `unscheduled_amount`,
		/// which can be used to repay more than the outstanding debt.
		/// The portfolio  valuation of the pool is updated to reflect the new
		/// present value of the loan.
		#[pallet::weight(T::WeightInfo::repay(T::MaxActiveLoansPerPool::get()))]
		#[pallet::call_index(2)]
		pub fn repay(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			amount: RepaidAmount<T::Balance>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let (amount, _count) = Self::update_active_loan(pool_id, loan_id, |loan| {
				Self::ensure_loan_borrower(&who, loan.borrower())?;
				loan.repay(amount.clone())
			})?;

			T::Pool::deposit(pool_id, who, amount.total()?)?;

			Self::deposit_event(Event::<T>::Repaid {
				pool_id,
				loan_id,
				amount,
			});

			Ok(())
		}

		/// Writes off an overdue loan.
		///
		/// This action will write off based on the configured write off policy.
		/// The write off action will only take effect if it writes down more
		/// (percentage or penalty) than the current write off status of the
		/// loan. This action will never writes up. i.e:
		/// - Write off by admin with percentage 0.5 and penalty 0.2
		/// - Time passes and the policy can be applied.
		/// - Write of with a policy that says: percentage 0.3, penaly 0.4
		/// - The loan is written off with the maximum between the policy and
		///   the current rule: percentage 0.5, penalty 0.4
		///
		/// No special permisions are required to this call.
		/// The portfolio valuation of the pool is updated to reflect the new
		/// present value of the loan.
		#[pallet::weight(T::WeightInfo::write_off(T::MaxActiveLoansPerPool::get()))]
		#[pallet::call_index(3)]
		pub fn write_off(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let (status, _count) = Self::update_active_loan(pool_id, loan_id, |loan| {
				let rule = Self::find_write_off_rule(pool_id, loan)?
					.ok_or(Error::<T>::NoValidWriteOffRule)?;
				let status = rule.status.compose_max(&loan.write_off_status());

				loan.write_off(&status)?;
				Ok(status)
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
		/// Forces a writing off of a loan if the `percentage` and `penalty`
		/// parameters respecting the policy values as the maximum.
		/// This action can write down/up the current write off status of the
		/// loan. If there is no active policy, an admin write off action can
		/// write up the write off status. But if there is a policy applied, the
		/// admin can only write up until the policy. Write down more than the
		/// policy is always allowed. The portfolio valuation of the pool is
		/// updated to reflect the new present value of the loan.
		#[pallet::weight(T::WeightInfo::admin_write_off(T::MaxActiveLoansPerPool::get()))]
		#[pallet::call_index(4)]
		pub fn admin_write_off(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			percentage: T::Rate,
			penalty: T::Rate,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::LoanAdmin)?;

			let status = WriteOffStatus {
				percentage,
				penalty,
			};

			let (_, _count) = Self::update_active_loan(pool_id, loan_id, |loan| {
				let rule = Self::find_write_off_rule(pool_id, loan)?;
				Self::ensure_admin_write_off(&status, rule)?;

				loan.write_off(&status)?;
				Ok(())
			})?;

			Self::deposit_event(Event::<T>::WrittenOff {
				pool_id,
				loan_id,
				status,
			});

			Ok(())
		}

		/// Propose a change.
		/// The change is not performed until you call
		/// [`Pallet::apply_loan_mutation()`].
		#[pallet::weight(T::WeightInfo::propose_loan_mutation(T::MaxActiveLoansPerPool::get()))]
		#[pallet::call_index(5)]
		pub fn propose_loan_mutation(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			mutation: LoanMutation<T::Rate>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::LoanAdmin)?;

			let (mut loan, _count) = Self::get_active_loan(pool_id, loan_id)?;
			transactional::with_transaction(|| {
				let result = loan.mutate_with(mutation.clone());

				// We do not want to apply the mutation,
				// only check if there is no error in applying it
				TransactionOutcome::Rollback(result)
			})?;

			T::ChangeGuard::note(pool_id, Change::Loan(loan_id, mutation).into())?;

			Ok(())
		}

		/// Apply a proposed change identified by a change id.
		/// It will only perform the change if the requirements for it
		/// are fulfilled.
		#[pallet::weight(T::WeightInfo::apply_loan_mutation(T::MaxActiveLoansPerPool::get()))]
		#[pallet::call_index(6)]
		pub fn apply_loan_mutation(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			change_id: T::Hash,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let Change::Loan(loan_id, mutation) = Self::get_released_change(pool_id, change_id)? else {
                Err(Error::<T>::UnrelatedChangeId)?
			};

			let (_, _count) = Self::update_active_loan(pool_id, loan_id, |loan| {
				loan.mutate_with(mutation.clone())
			})?;

			Self::deposit_event(Event::<T>::Mutated {
				pool_id,
				loan_id,
				mutation,
			});

			Ok(())
		}

		/// Closes a given loan
		///
		/// A loan only can be closed if it's fully repaid by the loan borrower.
		/// Closing a loan gives back the collateral used for the loan to the
		/// borrower .
		#[pallet::weight(T::WeightInfo::close(T::MaxActiveLoansPerPool::get()))]
		#[pallet::call_index(7)]
		pub fn close(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			loan_id: T::LoanId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let ((closed_loan, borrower), _count) = match CreatedLoan::<T>::take(pool_id, loan_id) {
				Some(created_loan) => (created_loan.close()?, Zero::zero()),
				None => {
					let (active_loan, count) = Self::take_active_loan(pool_id, loan_id)?;
					(active_loan.close(pool_id)?, count)
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

		/// Updates the write off policy with write off rules.
		///
		/// The write off policy is used to automatically set a write off
		/// minimum value to the loan.
		#[pallet::weight(T::WeightInfo::propose_write_off_policy())]
		#[pallet::call_index(8)]
		pub fn propose_write_off_policy(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			policy: BoundedVec<WriteOffRule<T::Rate>, T::MaxWriteOffPolicySize>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::PoolAdmin)?;
			Self::ensure_pool_exists(pool_id)?;

			T::ChangeGuard::note(pool_id, Change::Policy(policy).into())?;

			Ok(())
		}

		/// Apply a proposed change identified by a change id.
		/// It will only perform the change if the requirements for it
		/// are fulfilled.
		#[pallet::weight(T::WeightInfo::apply_write_off_policy())]
		#[pallet::call_index(9)]
		pub fn apply_write_off_policy(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			change_id: T::Hash,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let Change::Policy(policy) = Self::get_released_change(pool_id, change_id)? else {
                Err(Error::<T>::UnrelatedChangeId)?
			};

			WriteOffPolicy::<T>::insert(pool_id, policy.clone());

			Self::deposit_event(Event::<T>::WriteOffPolicyUpdated { pool_id, policy });

			Ok(())
		}

		/// Updates the porfolio valuation for the given pool
		#[pallet::weight(T::WeightInfo::update_portfolio_valuation(
			T::MaxActiveLoansPerPool::get()
		))]
		#[pallet::call_index(10)]
		pub fn update_portfolio_valuation(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;
			Self::ensure_pool_exists(pool_id)?;

			let (_, count) = Self::update_portfolio_valuation_for_pool(pool_id)?;

			Ok(Some(T::WeightInfo::update_portfolio_valuation(count)).into())
		}
	}

	/// Utility methods
	impl<T: Config> Pallet<T>
	where
		PriceCollectionOf<T>: DataCollection<T::PriceId, Data = PriceResultOf<T>>,
	{
		fn now() -> Moment {
			T::Time::now().as_secs()
		}

		fn ensure_role(pool_id: T::PoolId, who: &T::AccountId, role: PoolRole) -> DispatchResult {
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

		fn ensure_pool_exists(pool_id: T::PoolId) -> DispatchResult {
			ensure!(T::Pool::pool_exists(pool_id), Error::<T>::PoolNotFound);
			Ok(())
		}

		fn ensure_admin_write_off(
			status: &WriteOffStatus<T::Rate>,
			rule: Option<WriteOffRule<T::Rate>>,
		) -> DispatchResult {
			let limit = rule.map(|r| r.status).unwrap_or_else(|| status.clone());
			ensure!(
				status.percentage >= limit.percentage && status.penalty >= limit.penalty,
				Error::<T>::from(WrittenOffError::LessThanPolicy)
			);

			Ok(())
		}

		fn generate_loan_id(pool_id: T::PoolId) -> Result<T::LoanId, ArithmeticError> {
			LastLoanId::<T>::try_mutate(pool_id, |last_loan_id| {
				last_loan_id.ensure_add_assign(One::one())?;
				Ok(*last_loan_id)
			})
		}

		fn find_write_off_rule(
			pool_id: T::PoolId,
			loan: &ActiveLoan<T>,
		) -> Result<Option<WriteOffRule<T::Rate>>, DispatchError> {
			let rules = WriteOffPolicy::<T>::get(pool_id).into_iter();
			policy::find_rule(rules, |trigger| loan.check_write_off_trigger(trigger))
		}

		fn get_released_change(
			pool_id: T::PoolId,
			change_id: T::Hash,
		) -> Result<ChangeOf<T>, DispatchError> {
			T::ChangeGuard::released(pool_id, change_id)?
				.try_into()
				.map_err(|_| Error::<T>::NoLoanChangeId.into())
		}

		fn update_portfolio_valuation_for_pool(
			pool_id: T::PoolId,
		) -> Result<(T::Balance, u32), DispatchError> {
			let rates = T::InterestAccrual::rates();
			let prices = T::PriceRegistry::collection(&pool_id);
			let loans = ActiveLoans::<T>::get(pool_id);
			let values = loans
				.iter()
				.map(|(loan_id, loan)| Ok((*loan_id, loan.present_value_by(&rates, &prices)?)))
				.collect::<Result<Vec<_>, DispatchError>>()?;

			let portfolio = portfolio::PortfolioValuation::from_values(Self::now(), values)?;
			let valuation = portfolio.value();
			PortfolioValuation::<T>::insert(pool_id, portfolio);

			Self::deposit_event(Event::<T>::PortfolioValuationUpdated {
				pool_id,
				valuation,
				update_type: PortfolioValuationUpdateType::Exact,
			});

			Ok((valuation, loans.len() as u32))
		}

		fn insert_active_loan(
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			loan: ActiveLoan<T>,
		) -> Result<u32, DispatchError> {
			PortfolioValuation::<T>::try_mutate(pool_id, |portfolio| {
				portfolio.insert_elem(loan_id, loan.present_value()?)?;

				Self::deposit_event(Event::<T>::PortfolioValuationUpdated {
					pool_id,
					valuation: portfolio.value(),
					update_type: PortfolioValuationUpdateType::Inexact,
				});

				ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
					active_loans
						.try_push((loan_id, loan))
						.map_err(|_| Error::<T>::MaxActiveLoansReached)?;

					Ok(active_loans.len().ensure_into()?)
				})
			})
		}

		fn update_active_loan<F, R>(
			pool_id: T::PoolId,
			loan_id: T::LoanId,
			f: F,
		) -> Result<(R, u32), DispatchError>
		where
			F: FnOnce(&mut ActiveLoan<T>) -> Result<R, DispatchError>,
		{
			PortfolioValuation::<T>::try_mutate(pool_id, |portfolio| {
				ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
					let (_, loan) = active_loans
						.iter_mut()
						.find(|(id, _)| *id == loan_id)
						.ok_or(Error::<T>::LoanNotActiveOrNotFound)?;

					let result = f(loan)?;

					portfolio.update_elem(loan_id, loan.present_value()?)?;

					Self::deposit_event(Event::<T>::PortfolioValuationUpdated {
						pool_id,
						valuation: portfolio.value(),
						update_type: PortfolioValuationUpdateType::Inexact,
					});

					Ok((result, active_loans.len().ensure_into()?))
				})
			})
		}

		fn take_active_loan(
			pool_id: T::PoolId,
			loan_id: T::LoanId,
		) -> Result<(ActiveLoan<T>, u32), DispatchError> {
			ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
				let index = active_loans
					.iter()
					.position(|(id, _)| *id == loan_id)
					.ok_or(Error::<T>::LoanNotActiveOrNotFound)?;

				Ok((
					active_loans.swap_remove(index).1,
					active_loans.len().ensure_into()?,
				))
			})
		}

		fn get_active_loan(
			pool_id: T::PoolId,
			loan_id: T::LoanId,
		) -> Result<(ActiveLoan<T>, u32), DispatchError> {
			let active_loans = ActiveLoans::<T>::get(pool_id);
			let count = active_loans.len().ensure_into()?;
			let (_, loan) = active_loans
				.into_iter()
				.find(|(id, _)| *id == loan_id)
				.ok_or(Error::<T>::LoanNotActiveOrNotFound)?;

			Ok((loan, count))
		}

		#[cfg(feature = "runtime-benchmarks")]
		/// Set the maturity date of the loan to this instant.
		pub fn expire(pool_id: T::PoolId, loan_id: T::LoanId) -> DispatchResult {
			Self::update_active_loan(pool_id, loan_id, |loan| {
				loan.set_maturity(T::Time::now().as_secs());
				Ok(())
			})?;
			Ok(())
		}
	}

	// TODO: This implementation can be cleaned once #908 be solved
	impl<T: Config> PoolNAV<T::PoolId, T::Balance> for Pallet<T>
	where
		PriceCollectionOf<T>: DataCollection<T::PriceId, Data = PriceResultOf<T>>,
	{
		type ClassId = T::ItemId;
		type RuntimeOrigin = T::RuntimeOrigin;

		fn nav(pool_id: T::PoolId) -> Option<(T::Balance, Moment)> {
			let portfolio = PortfolioValuation::<T>::get(pool_id);
			Some((portfolio.value(), portfolio.last_updated()))
		}

		fn update_nav(pool_id: T::PoolId) -> Result<T::Balance, DispatchError> {
			Ok(Self::update_portfolio_valuation_for_pool(pool_id)?.0)
		}

		fn initialise(_: OriginFor<T>, _: T::PoolId, _: T::ItemId) -> DispatchResult {
			// This Loans implementation does not need to initialize explicitally.
			Ok(())
		}
	}
}
