use cfg_primitives::Moment;
use cfg_traits::ops::EnsureFixedPointNumber;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	storage::bounded_btree_set::BoundedBTreeSet,
	traits::tokens::{self},
	RuntimeDebug,
};
use scale_info::TypeInfo;
use sp_runtime::{traits::Get, ArithmeticError, FixedPointNumber, FixedPointOperand};
use sp_std::collections::btree_set::BTreeSet;
use strum::EnumCount;

/// Indicator of when the write off should be applied
#[derive(
	Encode,
	Decode,
	Clone,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	TypeInfo,
	RuntimeDebug,
	MaxEncodedLen,
	EnumCount,
)]
pub enum WriteOffTrigger {
	/// Number in days after the maturity date has passed
	PrincipalOverdueDays(u32),

	/// Seconds since the oracle valuation was last updated
	OracleValuationOutdated(Moment),
}

/// Wrapper type to identify equality berween kinds of triggers,
/// without taking into account their inner values
#[derive(Encode, Decode, Clone, Eq, PartialOrd, Ord, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct UniqueWriteOffTrigger(pub WriteOffTrigger);

impl PartialEq for UniqueWriteOffTrigger {
	fn eq(&self, other: &Self) -> bool {
		match self.0 {
			WriteOffTrigger::PrincipalOverdueDays(_) => {
				matches!(other.0, WriteOffTrigger::PrincipalOverdueDays(_))
			}
			WriteOffTrigger::OracleValuationOutdated(_) => {
				matches!(other.0, WriteOffTrigger::OracleValuationOutdated(_))
			}
		}
	}
}

impl From<WriteOffTrigger> for UniqueWriteOffTrigger {
	fn from(trigger: WriteOffTrigger) -> Self {
		UniqueWriteOffTrigger(trigger)
	}
}

/// Type representing the length of different trigger kinds
pub struct TriggerSize;

impl Get<u32> for TriggerSize {
	fn get() -> u32 {
		WriteOffTrigger::COUNT as u32
	}
}

/// The data structure for storing a specific write off policy
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct WriteOffRule<Rate> {
	/// If any of the triggers is valid, the write-off rule can be applied
	pub triggers: BoundedBTreeSet<UniqueWriteOffTrigger, TriggerSize>,

	/// Content of this write off rule to be applied
	pub status: WriteOffStatus<Rate>,
}

impl<Rate> WriteOffRule<Rate> {
	pub fn new(
		triggers: impl IntoIterator<Item = WriteOffTrigger>,
		percentage: Rate,
		penalty: Rate,
	) -> Self {
		Self {
			triggers: BTreeSet::from_iter(triggers.into_iter().map(UniqueWriteOffTrigger))
				.try_into()
				.expect("Cannot exist more unique triggers in a set than `TriggerSize`, qed"),
			status: WriteOffStatus {
				percentage,
				penalty,
			},
		}
	}

	pub fn has_trigger(&self, trigger: WriteOffTrigger) -> bool {
		self.triggers.contains(&UniqueWriteOffTrigger(trigger))
	}

	pub fn has_trigger_value(&self, trigger: WriteOffTrigger) -> bool {
		self.triggers
			.iter()
			.any(|unique_trigger| unique_trigger.0 == trigger)
	}
}

/// The status of the writen off
#[derive(
	Encode,
	Decode,
	Clone,
	PartialEq,
	Eq,
	Default,
	PartialOrd,
	Ord,
	TypeInfo,
	RuntimeDebug,
	MaxEncodedLen,
)]
pub struct WriteOffStatus<Rate> {
	/// Percentage of present value we are going to write off on a loan
	pub percentage: Rate,

	/// Additional interest that accrues on the written down loan as penalty
	pub penalty: Rate,
}

impl<Rate> WriteOffStatus<Rate>
where
	Rate: FixedPointNumber,
{
	pub fn write_down<Balance: tokens::Balance + FixedPointOperand>(
		&self,
		debt: Balance,
	) -> Result<Balance, ArithmeticError> {
		debt.ensure_sub(self.percentage.ensure_mul_int(debt)?)
	}

	pub fn compose_max(&self, other: &WriteOffStatus<Rate>) -> WriteOffStatus<Rate> {
		Self {
			percentage: self.percentage.max(other.percentage),
			penalty: self.penalty.max(other.penalty),
		}
	}

	pub fn is_none(&self) -> bool {
		self.percentage.is_zero() && self.penalty.is_zero()
	}
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn same_trigger_kinds() {
		let triggers: BoundedBTreeSet<UniqueWriteOffTrigger, TriggerSize> = BTreeSet::from_iter([
			UniqueWriteOffTrigger(WriteOffTrigger::PrincipalOverdueDays(1)),
			UniqueWriteOffTrigger(WriteOffTrigger::PrincipalOverdueDays(2)),
		])
		.try_into()
		.unwrap();

		assert_eq!(triggers.len(), 1);
	}

	#[test]
	fn different_trigger_kinds() {
		let triggers: BoundedBTreeSet<UniqueWriteOffTrigger, TriggerSize> = BTreeSet::from_iter([
			UniqueWriteOffTrigger(WriteOffTrigger::PrincipalOverdueDays(1)),
			UniqueWriteOffTrigger(WriteOffTrigger::OracleValuationOutdated(1)),
		])
		.try_into()
		.unwrap();

		assert_eq!(triggers.len(), 2);
	}
}
