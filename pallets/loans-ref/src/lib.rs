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
	traits::{
		tokens::{
			self,
			nonfungibles::{Inspect, Mutate, Transfer},
		},
		UnixTime,
	},
	RuntimeDebug, StorageHasher,
};
use pallet::*;
use scale_info::TypeInfo;
use sp_arithmetic::traits::checked_pow;
use sp_runtime::{
	traits::{BadOrigin, One},
	ArithmeticError, FixedPointNumber, FixedPointOperand,
};

const SECONDS_PER_DAY: Moment = 3600 * 24;
const SECONDS_PER_YEAR: Moment = SECONDS_PER_DAY * 365;

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

impl<Rate> ValuationMethod<Rate>
where
	Rate: FixedPointNumber,
{
	pub fn present_value<Balance: tokens::Balance + FixedPointOperand>(
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

	pub fn is_valid(&self) -> bool {
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
	UpToTotalBorrows { advance_rate: Rate },

	/// Ceiling computation using the outstanding debt
	UpToOutstandingDebt { advance_rate: Rate },
}

/// Specify how offer a loan can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BorrowRestrictions {
	/// TODO
	None,
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
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct LoanInfo<Asset, Balance, Rate> {
	/// Specify the repayments schedule of the loan
	schedule: RepaymentSchedule,

	/// Collateral used for this loan
	collateral: Asset,

	/// Value of the collateral used for this loan
	collateral_value: Balance,

	/// Valuation method of this loan
	valuation_method: ValuationMethod<Rate>,

	/// Restrictions of this loan
	restrictions: LoanRestrictions<Rate>,
}

/// Data containing a loan that has been created but is not active yet.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct CreatedLoan<Asset, Balance, Rate> {
	/// Loan information
	info: LoanInfo<Asset, Balance, Rate>,

	/// Interest rate per second
	interest_rate_per_sec: Rate,
}

/// Data containing an active loan.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct ActiveLoan<LoanId, Asset, Balance, Rate> {
	/// Id of this loan
	loan_id: LoanId,

	/// Loan information
	info: LoanInfo<Asset, Balance, Rate>,

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
pub struct ClosedLoan<BlockNumber, Asset, Balance, Rate> {
	/// Block when the loan was closed
	closed_at: BlockNumber,

	/// Loan information
	info: LoanInfo<Asset, Balance, Rate>,
}

type PoolIdOf<T> = <<T as Config>::Pool as PoolInspect<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
>>::PoolId;

type ItemIdOf<T> =
	<<T as Config>::NonFungible as Inspect<<T as frame_system::Config>::AccountId>>::ItemId;

type CollectionIdOf<T> =
	<<T as Config>::NonFungible as Inspect<<T as frame_system::Config>::AccountId>>::CollectionId;

type AssetOf<T> = (<T as Config>::CollectionId, <T as Config>::ItemId);

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{pallet_prelude::*, transactional};
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

		type CollectionId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Default
			+ TypeInfo
			+ Copy
			+ MaxEncodedLen;

		type ItemId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Default
			+ TypeInfo
			+ Copy
			+ MaxEncodedLen;

		type LoanId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Default
			+ TypeInfo
			+ MaxEncodedLen
			+ Copy
			+ AsRef<[u8]>;

		type Hasher: StorageHasher<Output = Self::LoanId>;

		type Rate: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ TypeInfo
			+ MaxEncodedLen;

		type Balance: tokens::Balance;

		/// An entity that can mint, transfer, and inspect assets.
		type NonFungible: Transfer<Self::AccountId>
			+ Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, CollectionId = Self::CollectionId, ItemId = Self::ItemId>;

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
		CreatedLoan<AssetOf<T>, T::Balance, T::Rate>,
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
			ActiveLoan<T::LoanId, AssetOf<T>, T::Balance, T::Rate>,
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
		ClosedLoan<T::BlockNumber, AssetOf<T>, T::Balance, T::Rate>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A loan was created.
		Created {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			loan_info: LoanInfo<AssetOf<T>, T::Balance, T::Rate>,
			interest_rate_per_sec: T::Rate,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when pool doesn't exist
		PoolMissing,
		/// Emits when the NFT owner is not found
		NFTOwnerNotFound,
		/// Emits when NFT owner doesn't match the expected owner
		NotAssetOwner,
		/// Emits when NFT the specified valuation method is not considered valid
		ValuationMethodNotValid,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn create(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_info: LoanInfo<AssetOf<T>, T::Balance, T::Rate>,
			interest_rate_per_year: T::Rate,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_role(pool_id, &who, PoolRole::Borrower)?;

			let (collection_id, item_id) = loan_info.collateral;
			let owner = T::NonFungible::owner(&collection_id, &item_id)
				.ok_or(Error::<T>::NFTOwnerNotFound)?;

			ensure!(who == owner, Error::<T>::NotAssetOwner);

			let loan_id = LastLoanId::<T>::mutate(|last_loan_id| {
				*last_loan_id = T::Hasher::hash(&*last_loan_id.as_ref());
				*last_loan_id
			});

			// CHECK: should we check more info from the `loan_info`?
			ensure!(
				loan_info.valuation_method.is_valid(),
				Error::<T>::ValuationMethodNotValid
			);

			let interest_rate_per_sec =
				T::InterestAccrual::reference_yearly_rate(interest_rate_per_year)?;

			CreatedLoans::<T>::insert(
				pool_id,
				loan_id,
				CreatedLoan {
					info: loan_info.clone(),
					interest_rate_per_sec,
				},
			);

			Self::deposit_event(Event::<T>::Created {
				pool_id,
				loan_id,
				loan_info,
				interest_rate_per_sec,
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
			// borrow_created()
			// or
			// borrow_active()
			todo!()
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn repay(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		#[transactional]
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
		#[transactional]
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

	/// Actions from extrinsics
	impl<T: Config> Pallet<T> {
		fn borrow_created() -> DispatchResult {
			todo!()
		}

		fn borrow_active() -> DispatchResult {
			todo!()
		}

		fn close_created() -> DispatchResult {
			todo!()
		}

		fn close_active() -> DispatchResult {
			todo!()
		}
	}

	/// Utility methods
	impl<T: Config> Pallet<T> {
		pub fn ensure_role(
			pool_id: PoolIdOf<T>,
			who: &T::AccountId,
			role: PoolRole,
		) -> Result<(), BadOrigin> {
			T::Permission::has(
				PermissionScope::Pool(pool_id),
				who.clone(),
				Role::PoolRole(role),
			)
			.then_some(())
			.ok_or(BadOrigin)
		}
	}
}
