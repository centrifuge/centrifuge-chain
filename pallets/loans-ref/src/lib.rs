#![cfg_attr(not(feature = "std"), no_std)]

use cfg_primitives::Moment;
use cfg_traits::{InterestAccrual, Permissions, PoolInspect, PoolReserve};
use cfg_types::{
	adjustments::Adjustment,
	permissions::{PermissionScope, Role},
};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	traits::{
		tokens::nonfungibles::{Inspect, Mutate, Transfer},
		UnixTime,
	},
	RuntimeDebug,
};
use pallet::*;
use scale_info::TypeInfo;
use sp_runtime::{traits::AtLeast32BitUnsigned, FixedPointOperand};

type PoolIdOf<T> = <<T as Config>::Pool as PoolInspect<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
>>::PoolId;

/// Two packed ids to represent an asset
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct Asset<ClassId, InstanceId>(
	/// Represents the asset collection
	pub ClassId,
	/// Represents the asset id
	pub InstanceId,
);

/// Diferent write off status that an unhealthy loan can have
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum WriteOffStatus<Rate> {
	/// Usual write off
	Standard {
		/// Index in the vec of write off groups.
		write_off_index: u32,
	},
	/// Written off by an admin
	Admin {
		/// Percentage of outstanding debt we are going to write off on a loan
		percentage: Rate,

		/// Additional interest that accrues on the written off loan as penalty
		penalty_interest_rate_per_sec: Rate,
	},
}

/// Defines the status of an active loan
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum Healthiness<Rate> {
	/// The loan has not been written off
	Healthy,

	/// The loan has been writen off
	Unhealthy(WriteOffStatus<Rate>),
}

/// Specify the expected repayments date
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum Maturity {
	/// Fixed point in time
	Fixed(Moment),
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

/// Diferents methods of how to compute the amount can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum MaxBorrowAmount<Rate> {
	/// Ceiling computation using the total borrow
	UpToTotalBorrows { advance_rate: Rate },

	/// Ceiling computation using the outstanding debt
	UpToOutstandingDebt { advance_rate: Rate },
}

/// Specify how offer a loan can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BorrowRestrictions {}

/// Specify how offer a loan can be repaid
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RepayRestrictions {}

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
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct LoanInfo<ClassId, LoanId, Balance, Rate> {
	/// Specify the repayments schedule of the loan
	schedule: RepaymentSchedule,

	/// Collateral used for this loan
	collateral: Asset<ClassId, LoanId>,

	/// Value of the collateral used for this loan
	collateral_value: Balance,

	/// Valuation method of this loan
	valuation_method: ValuationMethod<Rate>,

	/// Restrictions of this loan
	restrictions: LoanRestrictions<Rate>,
}

/// Data containing a loan that has been created but is not active yet.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct CreatedLoan<ClassId, LoanId, Balance, Rate> {
	/// Loan information
	info: LoanInfo<ClassId, LoanId, Balance, Rate>,

	/// Interest rate per second
	interest_rate_per_sec: Rate,
}

/// Data containing an active loan.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct ActiveLoan<ClassId, LoanId, Balance, Rate> {
	/// Id of this loan
	loan_id: LoanId,

	/// Loan information
	info: LoanInfo<ClassId, LoanId, Balance, Rate>,

	/// Specify whether the loan has been writen off
	healthiness: Healthiness<Rate>,

	/// Date when the loans becomes active
	origination_date: Moment,

	/// Normalized debt used to calculate the outstanding debt.
	normalized_debt: Balance,

	/// Total borrowed amount of this loan
	total_borrowed: Balance,

	/// Total repaid amount of this loan
	total_repaid: Balance,

	/// Interest rate per second
	interest_rate_per_sec: Rate,

	/// When the loans's Present Value (PV) was last updated
	last_updated: Moment,
}

/// Data containing a closed loan for historical purposes.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct ClosedLoan<BlockNumber, ClassId, LoanId, Balance, Rate> {
	/// Block when the loan was closed
	closed_at: BlockNumber,

	/// Loan information
	info: LoanInfo<ClassId, LoanId, Balance, Rate>,
}

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

		type ClassId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Copy
			+ Default
			+ TypeInfo
			+ MaxEncodedLen;

		type LoanId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Copy
			+ TypeInfo
			+ From<u128>
			+ MaxEncodedLen;

		type Rate: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ TypeInfo
			+ MaxEncodedLen;

		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u64>
			+ From<u128>
			+ TypeInfo
			+ TryInto<u64>;

		/// The NonFungible trait that can mint, transfer, and inspect assets.
		type NonFungible: Transfer<Self::AccountId>
			+ Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, ItemId = Self::LoanId, CollectionId = Self::ClassId>;

		/// Fetching method for the time of the current block
		type Time: UnixTime;

		/// Pool reserve type
		type Pool: PoolReserve<Self::AccountId, Self::CurrencyId, Balance = Self::Balance>;

		type CurrencyId: Parameter + Copy + MaxEncodedLen;

		/// Permission type that verifies permissions of users
		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<PoolIdOf<Self>, Self::CurrencyId>,
			Role = Role,
			Error = DispatchError,
		>;

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

	/// Storage for loans that has been created but are not still active.
	#[pallet::storage]
	#[pallet::getter(fn get_loan)]
	pub(crate) type CreatedLoans<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Blake2_128Concat,
		T::LoanId,
		LoanInfo<T::ClassId, T::LoanId, T::Balance, T::Rate>,
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
		BoundedVec<
			ActiveLoan<T::ClassId, T::LoanId, T::Balance, T::Rate>,
			T::MaxActiveLoansPerPool,
		>,
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
		ClosedLoan<T::BlockNumber, T::ClassId, T::LoanId, T::Balance, T::Rate>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			info: LoanInfo<T::ClassId, T::LoanId, T::Balance, T::Rate>,
			interest_rate_per_year: T::Rate,
		) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		pub fn borrow(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResult {
			// first_borrow()
			// or
			// borrow_again()
			todo!()
		}

		#[pallet::weight(10_000)]
		pub fn repay(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		pub fn write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		pub fn write_off_admin(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			percentage: T::Rate,
			penalty_interest_rate_per_year: T::Rate,
		) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		pub fn close(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			close: T::LoanId,
		) -> DispatchResult {
			// close_created()
			// or
			// close_active()
			todo!()
		}
	}

	impl<T: Config> Pallet<T> {
		fn first_borrow() -> DispatchResult {
			todo!()
		}

		fn borrow_again() -> DispatchResult {
			todo!()
		}

		fn close_created() -> DispatchResult {
			todo!()
		}

		fn close_active() -> DispatchResult {
			todo!()
		}
	}
}
