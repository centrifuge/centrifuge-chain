use cfg_traits::{
	self,
	data::DataCollection,
	interest::{InterestAccrual, InterestRate, RateCollection},
	IntoSeconds, Seconds, TimeAsSecs,
};
use cfg_types::adjustments::Adjustment;
use frame_support::{ensure, pallet_prelude::DispatchResult, RuntimeDebugNoBound};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		BlockNumberProvider, EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureSub, Zero,
	},
	DispatchError,
};

use crate::{
	entities::{
		changes::LoanMutation,
		input::{PrincipalInput, RepaidInput},
		pricing::{
			external::ExternalActivePricing, internal::InternalActivePricing, ActivePricing,
			Pricing,
		},
	},
	pallet::{AssetOf, Config, Error, PriceOf},
	types::{
		policy::{WriteOffStatus, WriteOffTrigger},
		BorrowLoanError, BorrowRestrictions, CloseLoanError, CreateLoanError, LoanRestrictions,
		MutationError, RepaidAmount, RepayLoanError, RepayRestrictions, RepaymentSchedule,
	},
};

/// Loan information.
/// It contemplates the loan proposal by the borrower and the pricing properties
/// by the issuer.
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct LoanInfo<T: Config> {
	/// Specify the repayments schedule of the loan
	pub schedule: RepaymentSchedule,

	/// Collateral used for this loan
	pub collateral: AssetOf<T>,

	/// Interest rate per year
	pub interest_rate: InterestRate<T::Rate>,

	/// Pricing properties for this loan
	pub pricing: Pricing<T>,

	/// Restrictions of this loan
	pub restrictions: LoanRestrictions,
}

impl<T: Config> LoanInfo<T> {
	pub fn collateral(&self) -> AssetOf<T> {
		self.collateral
	}

	/// Validates the loan information.
	pub fn validate(&self, now: Seconds) -> DispatchResult {
		match &self.pricing {
			Pricing::Internal(pricing) => pricing.validate()?,
			Pricing::External(pricing) => pricing.validate()?,
		}

		T::InterestAccrual::validate_rate(&self.interest_rate)?;

		ensure!(
			self.schedule.is_valid(now),
			Error::<T>::from(CreateLoanError::InvalidRepaymentSchedule)
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

impl<T: Config> CreatedLoan<T> {
	pub fn new(info: LoanInfo<T>, borrower: T::AccountId) -> Self {
		Self { info, borrower }
	}

	pub fn borrower(&self) -> &T::AccountId {
		&self.borrower
	}

	pub fn activate(
		self,
		pool_id: T::PoolId,
		initial_amount: PrincipalInput<T>,
	) -> Result<ActiveLoan<T>, DispatchError> {
		ActiveLoan::new(
			pool_id,
			self.info,
			self.borrower,
			initial_amount,
			T::Time::now(),
		)
	}

	pub fn close(self) -> Result<(ClosedLoan<T>, T::AccountId), DispatchError> {
		let loan = ClosedLoan {
			closed_at: frame_system::Pallet::<T>::current_block_number(),
			info: self.info,
			total_borrowed: Zero::zero(),
			total_repaid: Default::default(),
		};

		Ok((loan, self.borrower))
	}
}

/// Data containing a closed loan for historical purposes.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ClosedLoan<T: Config> {
	/// Block when the loan was closed
	closed_at: BlockNumberFor<T>,

	/// Loan information
	info: LoanInfo<T>,

	/// Total borrowed amount of this loan
	total_borrowed: T::Balance,

	/// Total repaid amount of this loan
	total_repaid: RepaidAmount<T::Balance>,
}

impl<T: Config> ClosedLoan<T> {
	pub fn collateral(&self) -> AssetOf<T> {
		self.info.collateral
	}
}

/// Data containing an active loan.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ActiveLoan<T: Config> {
	/// Specify the repayments schedule of the loan
	schedule: RepaymentSchedule,

	/// Collateral used for this loan
	collateral: AssetOf<T>,

	/// Restrictions of this loan
	restrictions: LoanRestrictions,

	/// Borrower account that created this loan
	borrower: T::AccountId,

	/// Write off percentage of this loan
	write_off_percentage: T::Rate,

	/// Date when the loans becomes active
	origination_date: Seconds,

	/// Pricing properties
	pricing: ActivePricing<T>,

	/// Total borrowed amount of this loan
	total_borrowed: T::Balance,

	/// Total repaid amount of this loan
	total_repaid: RepaidAmount<T::Balance>,

	/// Until this date all principal & interest
	/// payments occurred as scheduled.
	repayments_on_schedule_until: Seconds,
}

impl<T: Config> ActiveLoan<T> {
	pub fn new(
		pool_id: T::PoolId,
		info: LoanInfo<T>,
		borrower: T::AccountId,
		initial_amount: PrincipalInput<T>,
		now: Seconds,
	) -> Result<Self, DispatchError> {
		Ok(ActiveLoan {
			schedule: info.schedule,
			collateral: info.collateral,
			borrower,
			write_off_percentage: T::Rate::zero(),
			origination_date: now,
			pricing: match info.pricing {
				Pricing::Internal(inner) => ActivePricing::Internal(
					InternalActivePricing::activate(inner, info.interest_rate)?,
				),
				Pricing::External(inner) => {
					ActivePricing::External(ExternalActivePricing::activate(
						inner,
						info.interest_rate,
						pool_id,
						initial_amount.external()?,
						info.restrictions.borrows == BorrowRestrictions::OraclePriceRequired,
					)?)
				}
			},
			restrictions: info.restrictions,
			total_borrowed: T::Balance::zero(),
			total_repaid: RepaidAmount::default(),
			repayments_on_schedule_until: now,
		})
	}

	pub fn borrower(&self) -> &T::AccountId {
		&self.borrower
	}

	pub fn maturity_date(&self) -> Seconds {
		self.schedule.maturity.date()
	}

	pub fn pricing(&self) -> &ActivePricing<T> {
		&self.pricing
	}

	pub fn write_off_status(&self) -> WriteOffStatus<T::Rate> {
		WriteOffStatus {
			percentage: self.write_off_percentage,
			penalty: match &self.pricing {
				ActivePricing::Internal(inner) => inner.interest.penalty(),
				ActivePricing::External(inner) => inner.interest.penalty(),
			},
		}
	}

	fn write_down(&self, value: T::Balance) -> Result<T::Balance, DispatchError> {
		Ok(value.ensure_sub(self.write_off_percentage.ensure_mul_int(value)?)?)
	}

	/// Check if a write off rule is applicable for this loan
	pub fn check_write_off_trigger(
		&self,
		trigger: &WriteOffTrigger,
		pool_id: T::PoolId,
	) -> Result<bool, DispatchError> {
		let now = T::Time::now();
		match trigger {
			WriteOffTrigger::PrincipalOverdue(overdue_secs) => {
				Ok(now >= self.maturity_date().ensure_add(*overdue_secs)?)
			}
			WriteOffTrigger::PriceOutdated(secs) => match &self.pricing {
				ActivePricing::External(pricing) => Ok(now
					>= pricing
						.last_updated(pool_id)?
						.into_seconds()
						.ensure_add(*secs)?),
				ActivePricing::Internal(_) => Ok(false),
			},
		}
	}

	pub fn present_value(&self, pool_id: T::PoolId) -> Result<T::Balance, DispatchError> {
		let value = match &self.pricing {
			ActivePricing::Internal(inner) => {
				let maturity_date = self.schedule.maturity.date();
				inner.present_value(self.origination_date, maturity_date)?
			}
			ActivePricing::External(inner) => inner.present_value(pool_id)?,
		};

		self.write_down(value)
	}

	/// An optimized version of `ActiveLoan::present_value()` when some input
	/// data can be used from cached collections. Instead of fetch the current
	/// debt and prices from the pallets,
	/// it get the values from caches previously fetched.
	pub fn present_value_by<Rates, Prices>(
		&self,
		rates: &Rates,
		prices: &Prices,
	) -> Result<T::Balance, DispatchError>
	where
		Rates: RateCollection<T::Rate, T::Balance, T::Balance>,
		Prices: DataCollection<T::PriceId, Data = PriceOf<T>>,
	{
		let value = match &self.pricing {
			ActivePricing::Internal(inner) => {
				let maturity_date = self.schedule.maturity.date();
				inner.present_value_cached(rates, self.origination_date, maturity_date)?
			}
			ActivePricing::External(inner) => inner.present_value_cached(prices)?,
		};

		self.write_down(value)
	}

	fn ensure_can_borrow(&self, amount: &PrincipalInput<T>, pool_id: T::PoolId) -> DispatchResult {
		let max_borrow_amount = match &self.pricing {
			ActivePricing::Internal(inner) => {
				amount.internal()?;
				inner.max_borrow_amount(self.total_borrowed)?
			}
			ActivePricing::External(inner) => {
				let external_amount = amount.external()?;
				inner.max_borrow_amount(external_amount, pool_id)?
			}
		};

		ensure!(
			amount.balance()? <= max_borrow_amount,
			Error::<T>::from(BorrowLoanError::MaxAmountExceeded)
		);

		ensure!(
			match self.restrictions.borrows {
				BorrowRestrictions::NotWrittenOff => self.write_off_status().is_none(),
				BorrowRestrictions::FullOnce => {
					self.total_borrowed.is_zero() && amount.balance()? == max_borrow_amount
				}
				BorrowRestrictions::OraclePriceRequired => {
					match &self.pricing {
						ActivePricing::Internal(_) => true,
						ActivePricing::External(inner) => inner.last_updated(pool_id).is_ok(),
					}
				}
			},
			Error::<T>::from(BorrowLoanError::Restriction)
		);

		let now = T::Time::now();
		ensure!(
			self.schedule.maturity.is_valid(now),
			Error::<T>::from(BorrowLoanError::MaturityDatePassed)
		);

		Ok(())
	}

	pub fn borrow(&mut self, amount: &PrincipalInput<T>, pool_id: T::PoolId) -> DispatchResult {
		self.ensure_can_borrow(amount, pool_id)?;

		self.total_borrowed.ensure_add_assign(amount.balance()?)?;

		match &mut self.pricing {
			ActivePricing::Internal(inner) => {
				inner.adjust(Adjustment::Increase(amount.balance()?))?
			}
			ActivePricing::External(inner) => {
				inner.adjust(Adjustment::Increase(amount.external()?), Zero::zero())?
			}
		}

		Ok(())
	}

	/// Process the given amount to ensure it's a correct repayment.
	/// - Taking current interest accrued and maximal repay prinicpal from
	///   pricing
	/// - Adapting interest repayment to be as maximum as the current interest
	///   accrued
	/// - Checking repay restrictions
	fn prepare_repayment(
		&self,
		mut amount: RepaidInput<T>,
		pool_id: T::PoolId,
	) -> Result<RepaidInput<T>, DispatchError> {
		let (max_repay_principal, outstanding_interest) = match &self.pricing {
			ActivePricing::Internal(inner) => {
				amount.principal.internal()?;

				let principal = self
					.total_borrowed
					.ensure_sub(self.total_repaid.principal)?;

				(principal, inner.outstanding_interest(principal)?)
			}
			ActivePricing::External(inner) => {
				let external_amount = amount.principal.external()?;
				let max_repay_principal = inner.max_repay_principal(external_amount, pool_id)?;

				(max_repay_principal, inner.outstanding_interest()?)
			}
		};

		amount.interest = amount.interest.min(outstanding_interest);

		ensure!(
			amount.principal.balance()? <= max_repay_principal,
			Error::<T>::from(RepayLoanError::MaxPrincipalAmountExceeded)
		);

		ensure!(
			match self.restrictions.repayments {
				RepayRestrictions::None => true,
				RepayRestrictions::Full => {
					amount.principal.balance()? == max_repay_principal
						&& amount.interest == outstanding_interest
				}
			},
			Error::<T>::from(RepayLoanError::Restriction)
		);

		Ok(amount)
	}

	pub fn repay(
		&mut self,
		amount: RepaidInput<T>,
		pool_id: T::PoolId,
	) -> Result<RepaidInput<T>, DispatchError> {
		let amount = self.prepare_repayment(amount, pool_id)?;

		self.total_repaid
			.ensure_add_assign(&amount.repaid_amount()?)?;

		match &mut self.pricing {
			ActivePricing::Internal(inner) => {
				let amount = amount.repaid_amount()?.effective()?;
				inner.adjust(Adjustment::Decrease(amount))?
			}
			ActivePricing::External(inner) => {
				let principal = amount.principal.external()?;
				inner.adjust(Adjustment::Decrease(principal), amount.interest)?;
			}
		}

		Ok(amount)
	}

	pub fn write_off(&mut self, new_status: &WriteOffStatus<T::Rate>) -> DispatchResult {
		let penalty = new_status.penalty;
		match &mut self.pricing {
			ActivePricing::Internal(inner) => inner.interest.set_penalty(penalty)?,
			ActivePricing::External(inner) => inner.interest.set_penalty(penalty)?,
		}

		self.write_off_percentage = new_status.percentage;

		Ok(())
	}

	fn ensure_can_close(&self) -> DispatchResult {
		let can_close = match &self.pricing {
			ActivePricing::Internal(inner) => !inner.interest.has_debt(),
			ActivePricing::External(inner) => !inner.interest.has_debt(),
		};

		ensure!(can_close, Error::<T>::from(CloseLoanError::NotFullyRepaid));

		Ok(())
	}

	pub fn close(self, pool_id: T::PoolId) -> Result<(ClosedLoan<T>, T::AccountId), DispatchError> {
		self.ensure_can_close()?;

		let (pricing, interest_rate) = match self.pricing {
			ActivePricing::Internal(inner) => {
				let (pricing, interest_rate) = inner.deactivate()?;
				(Pricing::Internal(pricing), interest_rate)
			}
			ActivePricing::External(inner) => {
				let (pricing, interest_rate) = inner.deactivate(pool_id)?;
				(Pricing::External(pricing), interest_rate)
			}
		};

		let loan = ClosedLoan {
			closed_at: frame_system::Pallet::<T>::current_block_number(),
			info: LoanInfo {
				pricing,
				collateral: self.collateral,
				interest_rate,
				schedule: self.schedule,
				restrictions: self.restrictions,
			},
			total_borrowed: self.total_borrowed,
			total_repaid: self.total_repaid,
		};

		Ok((loan, self.borrower))
	}

	pub fn mutate_with(&mut self, mutation: LoanMutation<T::Rate>) -> DispatchResult {
		match mutation {
			LoanMutation::Maturity(maturity) => self.schedule.maturity = maturity,
			LoanMutation::MaturityExtension(extension) => self
				.schedule
				.maturity
				.extends(extension)
				.map_err(|_| Error::<T>::from(MutationError::MaturityExtendedTooMuch))?,
			LoanMutation::InterestRate(rate) => match &mut self.pricing {
				ActivePricing::Internal(inner) => inner.interest.set_base_rate(rate)?,
				ActivePricing::External(inner) => inner.interest.set_base_rate(rate)?,
			},
			LoanMutation::InterestPayments(payments) => self.schedule.interest_payments = payments,
			LoanMutation::PayDownSchedule(schedule) => self.schedule.pay_down_schedule = schedule,
			LoanMutation::Internal(mutation) => match &mut self.pricing {
				ActivePricing::Internal(inner) => inner.mutate_with(mutation)?,
				ActivePricing::External(_) => {
					Err(Error::<T>::from(MutationError::InternalPricingExpected))?
				}
			},
		};

		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	pub fn set_maturity(&mut self, duration: Seconds) {
		self.schedule.maturity = crate::types::Maturity::fixed(duration);
	}
}

/// Data containing an active loan with extra computed.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ActiveLoanInfo<T: Config> {
	/// Related active loan
	pub active_loan: ActiveLoan<T>,

	/// Present value of the loan
	pub present_value: T::Balance,

	/// Current outstanding principal of this loan
	pub outstanding_principal: T::Balance,

	/// Current outstanding interest of this loan
	pub outstanding_interest: T::Balance,
}

impl<T: Config> TryFrom<(T::PoolId, ActiveLoan<T>)> for ActiveLoanInfo<T> {
	type Error = DispatchError;

	fn try_from((pool_id, active_loan): (T::PoolId, ActiveLoan<T>)) -> Result<Self, Self::Error> {
		let (outstanding_principal, outstanding_interest) = match &active_loan.pricing {
			ActivePricing::Internal(inner) => {
				let principal = active_loan
					.total_borrowed
					.ensure_sub(active_loan.total_repaid.principal)?;

				(principal, inner.outstanding_interest(principal)?)
			}
			ActivePricing::External(inner) => (
				inner.outstanding_principal(pool_id)?,
				inner.outstanding_interest()?,
			),
		};

		Ok(Self {
			present_value: active_loan.present_value(pool_id)?,
			outstanding_principal,
			outstanding_interest,
			active_loan,
		})
	}
}
