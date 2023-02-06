#![cfg_attr(not(feature = "std"), no_std)]

mod types;

pub use pallet::*;

#[frame_support::pallet]
mod pallet {
	use cfg_primitives::Moment;
	use cfg_traits::{ops::EnsureAdd, InterestAccrual, Permissions, PoolInspect, PoolReserve};
	use cfg_types::{
		adjustments::Adjustment,
		permissions::{PermissionScope, PoolRole, Role},
	};
	use frame_support::{
		pallet_prelude::*,
		traits::{
			tokens::{
				self,
				nonfungibles::{Inspect, Mutate, Transfer},
			},
			UnixTime,
		},
		transactional, PalletError, StorageHasher,
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_arithmetic::FixedPointNumber;
	use sp_runtime::{
		traits::{BadOrigin, Zero},
		FixedPointOperand,
	};
	use types::{
		ActiveLoan, AssetOf, ClosedLoan, CreatedLoan, LoanInfo, LoanRestrictions,
		PortfolioValuation, PortfolioValuationUpdateType, RepaymentSchedule, ValuationMethod,
		WriteOffState, WriteOffStatus,
	};

	use super::*;

	type PoolIdOf<T> = <<T as Config>::Pool as PoolInspect<
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
			+ AsRef<[u8]>;

		/// Used to generate [`Self::LoanId`] identifiers
		type Hasher: StorageHasher<Output = Self::LoanId>;

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
			+ Mutate<Self::AccountId>
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
		type MaxWriteOffGroups: Get<u32>;
	}

	/// Contains the last loan id generated
	#[pallet::storage]
	pub(crate) type LastLoanId<T: Config> = StorageValue<_, T::LoanId, ValueQuery, GetDefault>;

	/// Storage for loans that has been created but are not still active.
	#[pallet::storage]
	pub(crate) type CreatedLoans<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Blake2_128Concat,
		T::LoanId,
		CreatedLoan<T>,
		OptionQuery,
	>;

	/// Storage for active loans.
	/// The indexation of this storage changes regarding the `CreatedLoans` or `ClosedLoans`
	/// because here we try to minimize the iteration speed over all active loans in a pool.
	#[pallet::storage]
	pub(crate) type ActiveLoans<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		BoundedVec<ActiveLoan<T>, T::MaxActiveLoansPerPool>,
		ValueQuery,
	>;

	/// Storage for closed loans.
	/// No mutations are expected in this storage.
	/// Loans are stored here for historical purposes.
	#[pallet::storage]
	pub(crate) type ClosedLoans<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Blake2_128Concat,
		T::LoanId,
		ClosedLoan<T>,
		OptionQuery,
	>;

	/// Stores write off policy used in each pool
	#[pallet::storage]
	pub(crate) type WriteOffPolicy<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		BoundedVec<WriteOffState<T::Rate>, T::MaxWriteOffGroups>,
		ValueQuery,
	>;

	/// Stores the portfolio valuation associated to each pool
	#[pallet::storage]
	pub(crate) type LatestPortfolioValuations<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolIdOf<T>, PortfolioValuation<T::Balance>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A loan was created
		Created {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			loan_info: LoanInfo<T>,
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
			value: T::Balance,
			update_type: PortfolioValuationUpdateType,
		},
		WriteOffPoliciesUpdated {
			pool_id: PoolIdOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when pool doesn't exist
		PoolNotFound,
		/// Emits when loan doesn't exist
		LoanNotFound,
		/// Emits when a write of state is not fount in a policy for a specific loan
		NoValidWriteOffState,
		/// Emits when the NFT owner is not found
		NFTOwnerNotFound,
		/// Emits when NFT owner doesn't match the expected owner
		NotNFTOwner,
		/// Emits when the applicant account is not the borrower of the loan
		NotLoanBorrower,
		/// Emits when the max number of active loans was reached
		MaxActiveLoansReached,
		/// Emits when the loan is bad specified and can not be created
		CreateLoanError(CreateLoanError),
		/// Emits when the loan can not be borrowed
		BorrowLoanError(BorrowLoanError),
		/// Emits when the loan can not be written off
		WrittenOffError(WrittenOffError),
		/// Emits when the loan can not be closed
		CloseLoanError(CloseLoanError),
	}

	/// Error related to loan creation
	#[derive(Encode, Decode, TypeInfo, PalletError)]
	pub enum CreateLoanError {
		/// Emits when valuation method is bad specified
		InvalidValuationMethod,
		/// Emits when repayment schedule is bad specified
		InvalidRepaymentSchedule,
	}

	impl<T> From<CreateLoanError> for Error<T> {
		fn from(error: CreateLoanError) -> Self {
			Error::<T>::CreateLoanError(error)
		}
	}

	/// Error related to loan borrowing
	#[derive(Encode, Decode, TypeInfo, PalletError)]
	pub enum BorrowLoanError {
		/// Emits when the borrowed amount is more than the allowed amount
		MaxAmountExceeded,
		/// Emits when the loan can not be borrowed because the loan is written off
		WrittenOffRestriction,
		/// Emits when maturity has passed and borrower tried to borrow more
		MaturityDatePassed,
	}

	impl<T> From<BorrowLoanError> for Error<T> {
		fn from(error: BorrowLoanError) -> Self {
			Error::<T>::BorrowLoanError(error)
		}
	}

	/// Error related to loan borrowing
	#[derive(Encode, Decode, TypeInfo, PalletError)]
	pub enum WrittenOffError {
		/// Emits when maturity has not passed tried to writ off
		MaturityDateNotPassed,
		/// Emits when a write off action tries to write off the more than the policy allows
		LessThanPolicy,
	}

	impl<T> From<WrittenOffError> for Error<T> {
		fn from(error: WrittenOffError) -> Self {
			Error::<T>::WrittenOffError(error)
		}
	}

	/// Error related to loan closing
	#[derive(Encode, Decode, TypeInfo, PalletError)]
	pub enum CloseLoanError {
		/// Emits when close a loan that is not fully repaid
		NotFullyRepaid,
	}

	impl<T> From<CloseLoanError> for Error<T> {
		fn from(error: CloseLoanError) -> Self {
			Error::<T>::CloseLoanError(error)
		}
	}

	/// Creates a new loan against the collateral provided
	///
	/// The origin must be the owner of the collateral.
	/// This collateral will be transferred to the existing pool.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn create(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			schedule: RepaymentSchedule,
			collateral: AssetOf<T>,
			collateral_value: T::Balance,
			valuation_method: ValuationMethod<T::Rate>,
			restrictions: LoanRestrictions<T::Rate>,
			interest_rate_per_year: T::Rate,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::Borrower)?;
			Self::ensure_collateral_owner(&who, collateral)?;
			Self::ensure_pool_exists(pool_id)?;

			let loan_info = LoanInfo::new(
				schedule,
				collateral,
				collateral_value,
				valuation_method,
				restrictions,
				interest_rate_per_year,
			)?;
			let loan_id = Self::generate_loan_id();

			T::NonFungible::transfer(&collateral.0, &collateral.1, &T::Pool::account_for(pool_id))?;

			CreatedLoans::<T>::insert(
				pool_id,
				loan_id,
				CreatedLoan {
					info: loan_info.clone(),
					borrower: who,
				},
			);

			Self::deposit_event(Event::<T>::Created {
				pool_id,
				loan_id,
				loan_info,
			});

			Ok(())
		}

		/// Transfers borrow amount to the borrower.
		///
		/// The origin must be the borrower of the loan.
		/// The borrow action should fulfill the borrow restrictions configured at [`LoanRestrictions`].
		/// The `amount` will be transferred from pool reserve to borrower.
		/// The portfolio valuation of the pool is updated to reflect the new present value of the loan.
		/// Rate accumulation will start after the first borrow.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn borrow(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			match CreatedLoans::<T>::take(pool_id, loan_id) {
				Some(created_loan) => {
					Self::ensure_loan_borrower(&who, &created_loan.borrower)?;
					Self::make_active_loan(pool_id, loan_id, created_loan, |loan| {
						loan.borrow(amount)
					})?
				}
				None => Self::update_active_loan(pool_id, loan_id, |loan| {
					Self::ensure_loan_borrower(&who, &loan.borrower())?;
					loan.borrow(amount)
				})?,
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
		/// The borrow action should fulfill the borrow restrictions configured at [`LoanRestrictions`].
		/// The `amount` will be transferred from borrower to pool reserve.
		/// The portfolio valuation of the pool is updated to reflect the new present value of the loan.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn repay(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let amount = Self::update_active_loan(pool_id, loan_id, |loan| {
				Self::ensure_loan_borrower(&who, &loan.borrower())?;
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
		/// No especial permisions are required to this call.
		/// The portfolio valuation of the pool is updated to reflect the new present value of the loan.
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let status = Self::update_active_loan(pool_id, loan_id, |loan| {
				let limit = Self::find_write_off_state(pool_id, loan.maturity_date())?;
				let status = limit.status();

				loan.write_off(&limit, &status)?;

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
		/// Forces a writing off of a loan if the `percentage` and `penalty` parameters
		/// respect the policy values as the minimum.
		/// The portfolio valuation of the pool is updated to reflect the new present value of the loan.
		#[pallet::weight(10_000)]
		#[transactional]
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

			Self::update_active_loan(pool_id, loan_id, |loan| {
				let limit = Self::find_write_off_state(pool_id, loan.maturity_date())?;
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
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn close(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let (info, borrower) = match CreatedLoans::<T>::take(pool_id, loan_id) {
				Some(loan) => (loan.info, loan.borrower),
				None => Self::take_active_loan(pool_id, loan_id)?.close()?,
			};

			Self::ensure_loan_borrower(&who, &borrower)?;

			let collateral = info.collateral();
			T::NonFungible::transfer(&collateral.0, &collateral.1, &who)?;

			ClosedLoans::<T>::insert(pool_id, loan_id, ClosedLoan::new(info)?);

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
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn update_write_off_policy(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			mut policy: BoundedVec<WriteOffState<T::Rate>, T::MaxWriteOffGroups>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::LoanAdmin)?;
			Self::ensure_pool_exists(pool_id)?;

			policy.iter_mut().try_for_each(|state| -> DispatchResult {
				state.penalty = Self::to_rate_per_sec(state.penalty)?;
				Ok(())
			})?;

			WriteOffPolicy::<T>::insert(pool_id, policy);

			Self::deposit_event(Event::<T>::WriteOffPoliciesUpdated { pool_id });

			Ok(())
		}

		/// Updates the porfolio valuation for the given pool
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn update_portfolio_valuation(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
		) -> DispatchResult {
			ensure_signed(origin)?;
			Self::ensure_pool_exists(pool_id)?;

			let value = Self::portfolio_valuation_for_pool(pool_id)?;

			LatestPortfolioValuations::<T>::insert(
				pool_id,
				PortfolioValuation::new(value, Self::now()),
			);

			Self::deposit_event(Event::<T>::PortfolioValuationUpdated {
				pool_id,
				value,
				update_type: PortfolioValuationUpdateType::Exact,
			});

			Ok(())
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
			.ok_or(BadOrigin.into())
		}

		fn ensure_collateral_owner(
			owner: &T::AccountId,
			(collection_id, item_id): AssetOf<T>,
		) -> DispatchResult {
			T::NonFungible::owner(&collection_id, &item_id)
				.ok_or(Error::<T>::NFTOwnerNotFound)?
				.eq(owner)
				.then_some(())
				.ok_or(Error::<T>::NotNFTOwner.into())
		}

		fn ensure_loan_borrower(owner: &T::AccountId, borrower: &T::AccountId) -> DispatchResult {
			ensure!(owner == borrower, Error::<T>::NotLoanBorrower);
			Ok(())
		}

		fn ensure_pool_exists(pool_id: PoolIdOf<T>) -> DispatchResult {
			ensure!(T::Pool::pool_exists(pool_id), Error::<T>::PoolNotFound);
			Ok(())
		}

		fn generate_loan_id() -> T::LoanId {
			LastLoanId::<T>::mutate(|last_loan_id| {
				*last_loan_id = T::Hasher::hash(&*last_loan_id.as_ref());
				*last_loan_id
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
			.ok_or(Error::<T>::NoValidWriteOffState.into())
		}

		fn update_portfolio_valuation_with_pv(
			pool_id: PoolIdOf<T>,
			portfolio: &mut PortfolioValuation<T::Balance>,
			old_pv: T::Balance,
			new_pv: T::Balance,
		) -> DispatchResult {
			let prev_value = portfolio.value();

			portfolio.update_with_pv_diff(old_pv, new_pv)?;

			if prev_value != portfolio.value() {
				Self::deposit_event(Event::<T>::PortfolioValuationUpdated {
					pool_id,
					value: portfolio.value(),
					update_type: PortfolioValuationUpdateType::Inexact,
				});
			}

			Ok(())
		}

		fn portfolio_valuation_for_pool(pool_id: PoolIdOf<T>) -> Result<T::Balance, DispatchError> {
			ActiveLoans::<T>::get(pool_id).into_iter().try_fold(
				T::Balance::zero(),
				|sum, loan| -> Result<T::Balance, DispatchError> {
					Ok(sum.ensure_add(loan.present_value()?)?)
				},
			)
		}

		fn make_active_loan<F, R>(
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			created_loan: CreatedLoan<T>,
			f: F,
		) -> Result<R, DispatchError>
		where
			F: FnOnce(&mut ActiveLoan<T>) -> Result<R, DispatchError>,
		{
			LatestPortfolioValuations::<T>::try_mutate(pool_id, |portfolio| {
				ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
					let mut loan = ActiveLoan::new(
						loan_id,
						created_loan.info,
						created_loan.borrower,
						Self::now(),
					);

					let result = f(&mut loan);
					let new_pv = loan.present_value()?;
					Self::update_portfolio_valuation_with_pv(
						pool_id,
						portfolio,
						Zero::zero(),
						new_pv,
					)?;

					active_loans
						.try_push(loan)
						.map_err(|_| Error::<T>::MaxActiveLoansReached)?;

					result
				})
			})
		}

		fn update_active_loan<F, R>(
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			f: F,
		) -> Result<R, DispatchError>
		where
			F: FnOnce(&mut ActiveLoan<T>) -> Result<R, DispatchError>,
		{
			LatestPortfolioValuations::<T>::try_mutate(pool_id, |portfolio| {
				ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
					let loan = active_loans
						.iter_mut()
						.find(|loan| loan.loan_id() == loan_id)
						.ok_or(Error::<T>::LoanNotFound)?;

					loan.update_time(portfolio.last_updated());
					let old_pv = loan.present_value()?;

					loan.update_time(Self::now());
					let result = f(loan);
					let new_pv = loan.present_value()?;

					Self::update_portfolio_valuation_with_pv(pool_id, portfolio, old_pv, new_pv)?;

					result
				})
			})
		}

		fn take_active_loan(
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> Result<ActiveLoan<T>, DispatchError> {
			ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
				let index = active_loans
					.iter()
					.position(|loan| loan.loan_id() == loan_id)
					.ok_or(Error::<T>::LoanNotFound)?;

				Ok(active_loans.swap_remove(index))
			})
		}
	}
}
