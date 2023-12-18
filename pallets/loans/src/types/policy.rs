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

use cfg_traits::Seconds;
use frame_support::{storage::bounded_btree_set::BoundedBTreeSet, RuntimeDebug};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{Get, Zero},
	DispatchError,
};
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
	/// Seconds after the maturity date has passed
	PrincipalOverdue(Seconds),

	/// Seconds since the oracle valuation was last updated
	PriceOutdated(Seconds),
}

/// Wrapper type to identify equality berween kinds of triggers,
/// without taking into account their inner values
#[derive(Encode, Decode, Clone, Eq, PartialOrd, Ord, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct UniqueWriteOffTrigger(pub WriteOffTrigger);

impl PartialEq for UniqueWriteOffTrigger {
	fn eq(&self, other: &Self) -> bool {
		match self.0 {
			WriteOffTrigger::PrincipalOverdue(_) => {
				matches!(other.0, WriteOffTrigger::PrincipalOverdue(_))
			}
			WriteOffTrigger::PriceOutdated(_) => {
				matches!(other.0, WriteOffTrigger::PriceOutdated(_))
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
	Rate: Ord + Zero + Copy,
{
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

/// From all overdue write off rules, it returns the one with the
/// highest percentage (or highest penalty, if same percentage) that can
/// be applied.
///
/// Suppose a policy with the following rules:
/// - overdue_secs: 5,   percentage 10%
/// - overdue_secs: 10,  percentage 30%
/// - overdue_secs: 15,  percentage 20%
///
/// If the loan is not overdue, it will not return any rule.
/// If the loan is overdue by 4 days, it will not return any rule.
/// If the loan is overdue by 9 days, it will return the first rule.
/// If the loan is overdue by 60 days, it will return the second rule
/// (because it has a higher percentage).
pub fn find_rule<Rate: Ord>(
	rules: impl Iterator<Item = WriteOffRule<Rate>>,
	has_effect: impl Fn(&WriteOffTrigger) -> Result<bool, DispatchError>,
) -> Result<Option<WriteOffRule<Rate>>, DispatchError> {
	// Get the triggered rules.
	let active_rules = rules
		.filter_map(|rule| {
			rule.triggers
				.iter()
				.map(|trigger| has_effect(&trigger.0))
				.find(|e| match e {
					Ok(value) => *value,
					Err(_) => true,
				})
				.map(|result| result.map(|_| rule))
		})
		.collect::<Result<sp_std::vec::Vec<_>, _>>()?; // Exits if error before getting the maximum

	// Get the rule with max percentage. If percentage are equals, max penaly.
	Ok(active_rules
		.into_iter()
		.max_by(|r1, r2| r1.status.cmp(&r2.status)))
}

#[cfg(test)]
mod tests {
	use frame_support::{assert_err, assert_ok};

	use super::*;

	#[test]
	fn same_trigger_kinds() {
		let triggers: BoundedBTreeSet<UniqueWriteOffTrigger, TriggerSize> = BTreeSet::from_iter([
			UniqueWriteOffTrigger(WriteOffTrigger::PrincipalOverdue(1)),
			UniqueWriteOffTrigger(WriteOffTrigger::PrincipalOverdue(2)),
		])
		.try_into()
		.unwrap();

		assert_eq!(triggers.len(), 1);
	}

	#[test]
	fn different_trigger_kinds() {
		let triggers: BoundedBTreeSet<UniqueWriteOffTrigger, TriggerSize> = BTreeSet::from_iter([
			UniqueWriteOffTrigger(WriteOffTrigger::PrincipalOverdue(1)),
			UniqueWriteOffTrigger(WriteOffTrigger::PriceOutdated(1)),
		])
		.try_into()
		.unwrap();

		assert_eq!(triggers.len(), 2);
	}

	#[test]
	fn find_correct_rule() {
		let rules = [
			WriteOffRule::new([WriteOffTrigger::PriceOutdated(0)], 5, 1),
			WriteOffRule::new([WriteOffTrigger::PriceOutdated(1)], 7, 1),
			WriteOffRule::new([WriteOffTrigger::PriceOutdated(2)], 7, 2), // <=
			WriteOffRule::new([WriteOffTrigger::PriceOutdated(3)], 3, 4),
			WriteOffRule::new([WriteOffTrigger::PriceOutdated(4)], 9, 1),
		];

		let expected = rules[2].clone();

		assert_ok!(
			find_rule(rules.into_iter(), |trigger| match trigger {
				WriteOffTrigger::PriceOutdated(secs) => Ok(*secs <= 3),
				_ => unreachable!(),
			}),
			Some(expected)
		);
	}

	#[test]
	fn find_err_rule() {
		let rules = [WriteOffRule::new([WriteOffTrigger::PriceOutdated(0)], 5, 1)];

		assert_err!(
			find_rule(rules.into_iter(), |trigger| match trigger {
				_ => Err(DispatchError::Other("")),
			}),
			DispatchError::Other("")
		);
	}

	#[test]
	fn find_none_rule() {
		let rules = [WriteOffRule::new([WriteOffTrigger::PriceOutdated(0)], 5, 1)];

		assert_ok!(
			find_rule(rules.into_iter(), |trigger| match trigger {
				_ => Ok(false),
			}),
			None
		);
	}
}
