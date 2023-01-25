#![cfg_attr(not(feature = "std"), no_std)]

use cfg_primitives::Moment;
use cfg_traits::{
	ops::{EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureMul, EnsureSub},
	InterestAccrual, Permissions, PoolInspect, PoolReserve,
};
use cfg_types::{
	adjustments::Adjustment,
	permissions::{PermissionScope, PoolRole, Role},
};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure,
	pallet_prelude::RuntimeDebugNoBound,
	traits::{
		tokens::{
			self,
			nonfungibles::{Inspect, Mutate, Transfer},
		},
		PalletError, UnixTime,
	},
	transactional, RuntimeDebug, StorageHasher,
};
use pallet::*;
use scale_info::TypeInfo;
use sp_arithmetic::traits::checked_pow;
use sp_runtime::{
	traits::{BadOrigin, BlockNumberProvider, One, Zero},
	ArithmeticError, FixedPointNumber, FixedPointOperand,
};

const SECONDS_PER_DAY: Moment = 3600 * 24;
const SECONDS_PER_YEAR: Moment = SECONDS_PER_DAY * 365;

/// The data structure for storing a specific write off policy
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct WriteOffPolicy<Rate> {
	/// Number in days after the maturity has passed at which this write off policy is valid
	overdue_days: u32,

	/// Percentage of present value we are going to write off on a loan
	percentage: Rate,

	/// Additional interest that accrues on the written off loan as penalty
	penalty_interest_rate_per_sec: Rate,
}

/// Diferent kinds of write off status that a loan can be
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum WriteOffStatus<Rate> {
	/// The loan has not been written down at all.
	None,

	/// Written down by a admin
	WrittenDownByPolicy {
		/// Percentage of present value we are going to write off on a loan
		percentage: Rate,

		/// Additional interest that accrues on the written down loan as penalty
		penalty_interest_rate_per_sec: Rate,
	},

	/// Written down by an admin
	WrittenDownByAdmin {
		/// Percentage of present value we are going to write off on a loan
		percentage: Rate,

		/// Additional interest that accrues on the written down loan as penalty
		penalty_interest_rate_per_sec: Rate,
	},

	/// Written down totally: 100% percentage, 0% penalty.
	WrittenOff,
}

/// Specify the expected repayments date
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum Maturity {
	/// Fixed point in time
	Fixed(Moment),
}

impl Maturity {
	fn date(&self) -> Moment {
		match self {
			Maturity::Fixed(moment) => *moment,
		}
	}
}

/// Interest payment periods
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestPayments {
	/// All interest is expected to be paid at the maturity date
	None,
}

/// Specify the paydown schedules of the loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum PayDownSchedule {
	/// The entire borrowed amount is expected to be paid back at the maturity date
	None,
}

/// Specify the repayment schedule of the loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct RepaymentSchedule {
	/// Expected repayments date for remaining debt
	maturity: Maturity,

	/// Period at which interest is paid
	interest_payments: InterestPayments,

	/// How much of the initially borrowed amount is paid back during interest payments
	pay_down_schedule: PayDownSchedule,
}

impl RepaymentSchedule {
	fn is_valid(&self, now: Moment) -> bool {
		self.maturity.date() > now
	}
}

/// Defines the valuation method of a loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum ValuationMethod<Rate> {
	/// TODO
	DiscountedCashFlows {
		/// TODO
		probability_of_default: Rate,

		/// TODO
		loss_given_default: Rate,

		/// TODO
		discount_rate: Rate,
	},
	/// TODO
	OutstandingDebt,
}

impl<Rate> ValuationMethod<Rate>
where
	Rate: FixedPointNumber,
{
	fn present_value<Balance: tokens::Balance + FixedPointOperand>(
		&self,
		debt: Balance,
		maturity_date: Moment,
		origination_date: Moment,
		now: Moment,
		interest_rate_per_sec: Rate,
	) -> Result<Balance, ArithmeticError> {
		match self {
			ValuationMethod::DiscountedCashFlows {
				loss_given_default,
				probability_of_default,
				discount_rate,
			} => {
				// If the loan is overdue, there are no future cash flows to discount,
				// hence we use the outstanding debt as the value.
				if now > maturity_date {
					return Ok(debt);
				}

				// Calculate the expected loss over the term of the loan
				let tel = Rate::saturating_from_rational(
					maturity_date.ensure_sub(origination_date)?,
					SECONDS_PER_YEAR,
				)
				.ensure_mul(*probability_of_default)?
				.ensure_mul(*loss_given_default)?
				.min(One::one());

				let tel_inv = Rate::one().ensure_sub(tel)?;

				// Calculate the risk-adjusted expected cash flows
				let exp = maturity_date.ensure_sub(now)?.ensure_into()?;
				let acc_rate =
					checked_pow(interest_rate_per_sec, exp).ok_or(ArithmeticError::Overflow)?;
				let ecf = acc_rate.ensure_mul_int(debt)?;
				let ra_ecf = tel_inv.ensure_mul_int(ecf)?;

				// Discount the risk-adjusted expected cash flows
				let rate = checked_pow(*discount_rate, exp).ok_or(ArithmeticError::Overflow)?;
				let d = Rate::one().ensure_div(rate)?;

				d.ensure_mul_int(ra_ecf)
			}
			ValuationMethod::OutstandingDebt => Ok(debt),
		}
	}

	fn is_valid(&self) -> bool {
		match self {
			ValuationMethod::DiscountedCashFlows { discount_rate, .. } => {
				discount_rate >= &One::one()
			}
			ValuationMethod::OutstandingDebt => true,
		}
	}
}

/// Diferents methods of how to compute the amount can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum MaxBorrowAmount<Rate> {
	/// Ceiling computation using the total borrow
	UpToTotalBorrowed { advance_rate: Rate },

	/// Ceiling computation using the outstanding debt
	UpToOutstandingDebt { advance_rate: Rate },
}

/// Specify how offer a loan can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BorrowRestrictions {
	/// The loan can be borrowed if it is not written down.
	WrittenDown,
}

/// Specify how offer a loan can be repaid
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RepayRestrictions {
	/// TODO
	None,
}

/// Define the loan restrictions
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct LoanRestrictions<Rate> {
	/// How much can be borrowed
	max_borrow_amount: MaxBorrowAmount<Rate>,

	/// How offen can be borrowed
	borrows: BorrowRestrictions,

	/// How offen can be repaid
	repayments: RepayRestrictions,
}

/// Loan information.
/// It contemplates the loan proposal by the borrower and the pricing properties by the issuer.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct LoanInfo<T: Config> {
	/// Specify the repayments schedule of the loan
	schedule: RepaymentSchedule,

	/// Collateral used for this loan
	collateral: AssetOf<T>,

	/// Value of the collateral used for this loan
	collateral_value: T::Balance,

	/// Valuation method of this loan
	valuation_method: ValuationMethod<T::Rate>,

	/// Restrictions of this loan
	restrictions: LoanRestrictions<T::Rate>,

	/// Interest rate per second
	interest_rate_per_sec: T::Rate,
}

impl<T: Config> LoanInfo<T> {
	fn validate(&self, now: Moment) -> sp_runtime::DispatchResult {
		ensure!(
			self.valuation_method.is_valid(),
			Error::<T>::from(InnerLoanError::ValuationMethod)
		);

		ensure!(
			self.schedule.is_valid(now),
			Error::<T>::from(InnerLoanError::RepaymentSchedule)
		);

		Ok(())
	}
}

/// Data containing a loan that has been created but is not active yet.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct CreatedLoan<T: Config> {
	/// Loan information
	info: LoanInfo<T>,

	/// Borrower account that created this loan
	borrower: T::AccountId,
}

/// Data containing an active loan.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ActiveLoan<T: Config> {
	/// Id of this loan
	loan_id: T::LoanId,

	/// Loan information
	info: LoanInfo<T>,

	/// Borrower account that created this loan
	borrower: T::AccountId,

	/// Specify whether the loan has been writen off
	written_off_status: WriteOffStatus<T::Rate>,

	/// Date when the loans becomes active
	origination_date: Moment,

	/// Normalized debt used to calculate the outstanding debt.
	normalized_debt: T::Balance,

	/// Total borrowed amount of this loan
	total_borrowed: T::Balance,

	/// Total repaid amount of this loan
	total_repaid: T::Balance,

	/// When the loans's Present Value (PV) was last updated
	last_updated: Moment,
}

/// Data containing a closed loan for historical purposes.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ClosedLoan<T: Config> {
	/// Block when the loan was closed
	closed_at: T::BlockNumber,

	/// Loan information
	info: LoanInfo<T>,
}

#[derive(Encode, Decode, TypeInfo)]
pub enum InnerLoanError {
	ValuationMethod,
	RepaymentSchedule,
}

impl PalletError for InnerLoanError {
	const MAX_ENCODED_SIZE: usize = 1; // Up to 256 errors
}

impl<T> From<InnerLoanError> for Error<T> {
	fn from(error: InnerLoanError) -> Self {
		Error::<T>::InvalidLoanValue(error)
	}
}

type PoolIdOf<T> = <<T as Config>::Pool as PoolInspect<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
>>::PoolId;

type AssetOf<T> = (<T as Config>::CollectionId, <T as Config>::ItemId);

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_arithmetic::FixedPointNumber;

	use super::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Identify a curreny.
		/// Used to specify [`PoolReserve`] and [`Permisions`].
		type CurrencyId: Parameter + Copy + MaxEncodedLen;

		/// Identify an non fungible collection
		type CollectionId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Default
			+ TypeInfo
			+ Copy
			+ MaxEncodedLen;

		/// Identify an non fungible item
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

		/// Used to generate [`LoanId`] identifiers
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

	#[pallet::storage]
	pub(crate) type LastLoanId<T: Config> = StorageValue<_, T::LoanId, ValueQuery, GetDefault>;

	/// Storage for loans that has been created but are not still active.
	#[pallet::storage]
	#[pallet::getter(fn get_loan)]
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

	/// Stores write off policies used in each pool
	#[pallet::storage]
	#[pallet::getter(fn pool_writeoff_groups)]
	pub(crate) type WriteOffPolicies<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		BoundedVec<WriteOffPolicy<T::Rate>, T::MaxWriteOffGroups>,
		ValueQuery,
	>;

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
		},
		/// A loan was closed
		Closed {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			collateral: AssetOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when pool doesn't exist
		PoolNotFound,
		/// Emits when the NFT owner is not found
		NFTOwnerNotFound,
		/// Emits when NFT owner doesn't match the expected owner
		NotNFTOwner,
		/// Emits when the loan is bad specified
		InvalidLoanValue(InnerLoanError),
		/// Emits when loan doesn't exist
		LoanNotFound,
		/// Emits when the applicant account is not the borrower of the loan
		NotLoanBorrower,
		/// Emits when the max number of active loans was reached
		MaxActiveLoansReached,
	}

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

			ensure!(T::Pool::pool_exists(pool_id), Error::<T>::PoolNotFound);

			let loan_info = LoanInfo {
				schedule,
				collateral,
				collateral_value,
				valuation_method,
				restrictions,
				interest_rate_per_sec: T::InterestAccrual::reference_yearly_rate(
					interest_rate_per_year,
				)?,
			};

			loan_info.validate(Self::now())?;

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
				Some(loan) => {
					Self::ensure_loan_borrower(&who, &loan.borrower)?;
					Self::make_active_loan(pool_id, loan_id, loan.info, loan.borrower, |loan| {
						Self::do_borrow(loan, amount)
					})?
				}
				None => Self::mutate_active_loan(pool_id, loan_id, |loan| {
					Self::ensure_loan_borrower(&who, &loan.borrower)?;
					Self::do_borrow(loan, amount)
				})?,
			}

			Self::deposit_event(Event::<T>::Borrowed {
				pool_id,
				loan_id,
				amount,
			});

			Ok(())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn repay(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::mutate_active_loan(pool_id, loan_id, |loan| {
				Self::ensure_loan_borrower(&who, &loan.borrower)?;
				Self::do_repay(loan, amount)
			})?;

			Self::deposit_event(Event::<T>::Repaid {
				pool_id,
				loan_id,
				amount,
			});

			Ok(())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::mutate_active_loan(pool_id, loan_id, |loan| {
				Self::ensure_loan_borrower(&who, &loan.borrower)?;
				Self::do_write_off(loan)
			})?;

			Self::deposit_event(Event::<T>::WrittenOff { pool_id, loan_id });

			Ok(())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn close(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let info = match CreatedLoans::<T>::take(pool_id, loan_id) {
				Some(loan) => loan.info,
				None => Self::take_active_loan(pool_id, loan_id, |loan| {
					Self::ensure_loan_borrower(&who, &loan.borrower)?;
					Self::do_close(loan)?;
					Ok(loan.info.clone())
				})?,
			};

			ClosedLoans::<T>::insert(
				pool_id,
				loan_id,
				ClosedLoan {
					closed_at: frame_system::Pallet::<T>::current_block_number(),
					info: info.clone(),
				},
			);

			Self::deposit_event(Event::<T>::Closed {
				pool_id,
				loan_id,
				collateral: info.collateral,
			});

			Ok(())
		}
	}

	/// Active loan actions
	impl<T: Config> Pallet<T> {
		fn do_borrow(loan: &mut ActiveLoan<T>, amount: T::Balance) -> DispatchResult {
			todo!()
		}

		fn do_repay(loan: &mut ActiveLoan<T>, amount: T::Balance) -> DispatchResult {
			todo!()
		}

		fn do_write_off(loan: &mut ActiveLoan<T>) -> DispatchResult {
			todo!()
		}

		fn do_close(loan: &mut ActiveLoan<T>) -> DispatchResult {
			todo!()
		}
	}

	/// Utility methods
	impl<T: Config> Pallet<T> {
		fn now() -> Moment {
			T::Time::now().as_secs()
		}

		fn ensure_role(
			pool_id: PoolIdOf<T>,
			who: &T::AccountId,
			role: PoolRole,
		) -> Result<(), BadOrigin> {
			T::Permissions::has(
				PermissionScope::Pool(pool_id),
				who.clone(),
				Role::PoolRole(role),
			)
			.then_some(())
			.ok_or(BadOrigin)
		}

		fn ensure_collateral_owner(
			owner: &T::AccountId,
			(collection_id, item_id): AssetOf<T>,
		) -> Result<(), Error<T>> {
			T::NonFungible::owner(&collection_id, &item_id)
				.ok_or(Error::<T>::NFTOwnerNotFound)?
				.eq(owner)
				.then_some(())
				.ok_or(Error::<T>::NotNFTOwner)
		}

		fn ensure_loan_borrower(
			owner: &T::AccountId,
			borrower: &T::AccountId,
		) -> Result<(), Error<T>> {
			ensure!(owner == borrower, Error::<T>::NotLoanBorrower);
			Ok(())
		}

		fn generate_loan_id() -> T::LoanId {
			LastLoanId::<T>::mutate(|last_loan_id| {
				*last_loan_id = T::Hasher::hash(&*last_loan_id.as_ref());
				*last_loan_id
			})
		}

		fn make_active_loan<F, R>(
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			info: LoanInfo<T>,
			borrower: T::AccountId,
			f: F,
		) -> Result<R, DispatchError>
		where
			F: FnOnce(&mut ActiveLoan<T>) -> Result<R, DispatchError>,
		{
			ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
				let index = active_loans.len();
				active_loans
					.try_push(ActiveLoan {
						loan_id,
						info,
						borrower,
						written_off_status: WriteOffStatus::None,
						origination_date: 0,
						normalized_debt: T::Balance::zero(),
						total_borrowed: T::Balance::zero(),
						total_repaid: T::Balance::zero(),
						last_updated: 0,
					})
					.map_err(|_| Error::<T>::MaxActiveLoansReached)?;

				f(active_loans
					.get_mut(index)
					.ok_or(DispatchError::Other("Expect an active loan at given index"))?)
			})
		}

		fn mutate_active_loan<F, R>(
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			f: F,
		) -> Result<R, DispatchError>
		where
			F: FnOnce(&mut ActiveLoan<T>) -> Result<R, DispatchError>,
		{
			ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
				let active_loan = active_loans
					.iter_mut()
					.find(|active_loan| active_loan.loan_id == loan_id)
					.ok_or(Error::<T>::LoanNotFound)?;

				f(active_loan)
			})
		}

		fn take_active_loan<F, R>(
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			f: F,
		) -> Result<R, DispatchError>
		where
			F: FnOnce(&mut ActiveLoan<T>) -> Result<R, DispatchError>,
		{
			ActiveLoans::<T>::try_mutate(pool_id, |active_loans| {
				let index = active_loans
					.iter()
					.position(|active_loan| active_loan.loan_id == loan_id)
					.ok_or(Error::<T>::LoanNotFound)?;

				f(&mut active_loans.swap_remove(index))
			})
		}
	}
}
