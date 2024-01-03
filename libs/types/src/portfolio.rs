// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::{Seconds, TimeAsSecs};
use frame_support::{traits::Get, BoundedVec, RuntimeDebug};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{EnsureAdd, EnsureSub, Zero},
	DispatchError, DispatchResult,
};
use sp_std::{cmp::Ordering, marker::PhantomData, vec::Vec};

// Portfolio valuation information.
// It will be updated on these scenarios:
//   1. When we are calculating portfolio valuation for a pool.
//   2. When there is borrow or repay or write off on a loan under this pool
//   3. When pool fee disbursement is prepared
// So the portfolio valuation could be:
// 	 - Approximate when current time != last_updated
// 	 - Exact when current time == last_updated
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxElems))]
pub struct PortfolioValuation<Balance, ElemId, MaxElems: Get<u32>> {
	/// Computed portfolio valuation for the given pool
	value: Balance,

	/// Last time when the portfolio valuation was calculated for the entire
	/// pool.
	last_updated: Seconds,

	/// Individual valuation of each element that compose the value of the
	/// portfolio
	values: BoundedVec<(ElemId, Balance), MaxElems>,
}

impl<Balance, ElemId, MaxElems> PortfolioValuation<Balance, ElemId, MaxElems>
where
	Balance: EnsureAdd + EnsureSub + Ord + Copy,
	ElemId: Eq,
	MaxElems: Get<u32>,
{
	pub fn new(when: Seconds) -> Self {
		Self {
			value: Balance::zero(),
			last_updated: when,
			values: BoundedVec::default(),
		}
	}

	pub fn from_values(
		when: Seconds,
		values: Vec<(ElemId, Balance)>,
	) -> Result<Self, DispatchError> {
		Ok(Self {
			value: values.iter().try_fold(
				Balance::zero(),
				|sum, (_, value)| -> Result<Balance, DispatchError> { Ok(sum.ensure_add(*value)?) },
			)?,
			values: values
				.try_into()
				.map_err(|_| DispatchError::Other("Max portfolio size reached"))?,
			last_updated: when,
		})
	}

	pub fn value(&self) -> Balance {
		self.value
	}

	pub fn last_updated(&self) -> Seconds {
		self.last_updated
	}

	pub fn value_of(&self, id: ElemId) -> Option<Balance> {
		self.values
			.iter()
			.find(|(elem_id, _)| *elem_id == id)
			.map(|(_, balance)| *balance)
	}

	pub fn insert_elem(&mut self, id: ElemId, pv: Balance) -> DispatchResult {
		self.values
			.try_push((id, pv))
			.map_err(|_| DispatchError::Other("Max portfolio size reached"))?;

		self.value.ensure_add_assign(pv)?;
		Ok(())
	}

	pub fn update_elem(&mut self, id: ElemId, new_pv: Balance) -> DispatchResult {
		let old_pv = self
			.values
			.iter_mut()
			.find(|(elem_id, _)| *elem_id == id)
			.map(|(_, value)| value)
			.ok_or(DispatchError::CannotLookup)?;

		match new_pv.cmp(old_pv) {
			Ordering::Greater => {
				let diff = new_pv.ensure_sub(*old_pv)?;
				self.value.ensure_add_assign(diff)?;
			}
			Ordering::Less => {
				let diff = old_pv.ensure_sub(new_pv)?;
				self.value.ensure_sub_assign(diff)?;
			}
			Ordering::Equal => (),
		};

		*old_pv = new_pv;

		Ok(())
	}

	pub fn remove_elem(&mut self, elem_id: ElemId) -> DispatchResult {
		let index = self
			.values
			.iter()
			.position(|(id, _)| *id == elem_id)
			.ok_or(DispatchError::CannotLookup)?;

		let (_, pv) = self.values.swap_remove(index);
		self.value.ensure_sub_assign(pv)?;
		Ok(())
	}
}

/// Type that builds a PortfolioValuation with the current instant.
pub struct InitialPortfolioValuation<Timer>(PhantomData<Timer>);

impl<Balance, ElemId, MaxElems, Timer> Get<PortfolioValuation<Balance, ElemId, MaxElems>>
	for InitialPortfolioValuation<Timer>
where
	Balance: Zero + EnsureAdd + EnsureSub + Ord + Copy,
	MaxElems: Get<u32>,
	Timer: TimeAsSecs,
	ElemId: Eq,
{
	fn get() -> PortfolioValuation<Balance, ElemId, MaxElems> {
		PortfolioValuation::new(<Timer as TimeAsSecs>::now())
	}
}

/// Information about how the portfolio valuation was updated
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum PortfolioValuationUpdateType {
	/// Portfolio Valuation was fully recomputed to an exact value
	Exact,
	/// Portfolio Valuation was updated inexactly based on status changes
	Inexact,
}

#[cfg(test)]
mod tests {
	use frame_support::assert_ok;
	use sp_core::ConstU32;

	use super::*;

	#[test]
	fn general_usage() {
		let mut portfolio = PortfolioValuation::<u128, u64, ConstU32<3>>::new(10);

		assert_ok!(portfolio.insert_elem(1, 100));
		assert_ok!(portfolio.insert_elem(2, 200));
		assert_ok!(portfolio.insert_elem(3, 300));

		assert_eq!(portfolio.value(), 600);

		// Increase
		assert_ok!(portfolio.update_elem(1, 300));
		assert_eq!(portfolio.value(), 800);

		// Do not change
		assert_ok!(portfolio.update_elem(2, 200));
		assert_eq!(portfolio.value(), 800);

		// Decrease
		assert_ok!(portfolio.update_elem(3, 100));
		assert_eq!(portfolio.value(), 600);

		assert_eq!(portfolio.value_of(1), Some(300));
		assert_eq!(portfolio.value_of(2), Some(200));
		assert_eq!(portfolio.value_of(3), Some(100));

		assert_ok!(portfolio.remove_elem(1));
		assert_ok!(portfolio.remove_elem(2));
		assert_ok!(portfolio.remove_elem(3));
		assert_eq!(portfolio.value(), 0);
	}
}
