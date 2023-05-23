use cfg_primitives::Moment;
use cfg_traits::ops::{EnsureAdd, EnsureAddAssign, EnsureSub, EnsureSubAssign};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::Get, BoundedVec, RuntimeDebug};
use scale_info::TypeInfo;
use sp_runtime::{traits::Zero, DispatchError, DispatchResult};
use sp_std::{cmp::Ordering, vec::Vec};

// Portfolio valuation information.
// It will be updated on these scenarios:
//   1. When we are calculating portfolio valuation for a pool.
//   2. When there is borrow or repay or write off on a loan under this pool
// So the portfolio valuation could be:
// 	 - Approximate when current time != last_updated
// 	 - Exact when current time == last_updated
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxElems))]
pub struct PortfolioValuation<Balance, ElemId, MaxElems: Get<u32>> {
	/// Computed portfolio valuation for the given pool
	value: Balance,

	/// Last time when the portfolio valuation was calculated for the entire
	/// pool. None if never has been computed entirely.
	last_updated: Option<Moment>,

	/// Individual valuation of each element that compose the value of the
	/// portfolio
	values: BoundedVec<(ElemId, Balance), MaxElems>,
}

impl<Balance, ElemId, MaxElems> Default for PortfolioValuation<Balance, ElemId, MaxElems>
where
	Balance: Zero,
	MaxElems: Get<u32>,
{
	fn default() -> Self {
		Self {
			value: Balance::zero(),
			last_updated: None,
			values: BoundedVec::default(),
		}
	}
}

impl<Balance, ElemId, MaxElems> PortfolioValuation<Balance, ElemId, MaxElems>
where
	Balance: EnsureAdd + EnsureSub + Ord + Copy,
	ElemId: Eq,
	MaxElems: Get<u32>,
{
	pub fn value(&self) -> Balance {
		self.value
	}

	pub fn last_updated(&self) -> Option<Moment> {
		self.last_updated
	}

	pub fn value_of(&self, id: ElemId) -> Option<&Balance> {
		self.values
			.iter()
			.find(|(elem_id, _)| *elem_id == id)
			.map(|(_, balance)| balance)
	}

	pub fn update(
		&mut self,
		pv_list: Vec<(ElemId, Balance)>,
		when: Moment,
	) -> Result<Balance, DispatchError> {
		self.values = pv_list
			.try_into()
			.map_err(|_| DispatchError::Other("TODO"))?;

		self.value = self.values.iter().try_fold(
			Balance::zero(),
			|sum, (_, value)| -> Result<Balance, DispatchError> { Ok(sum.ensure_add(*value)?) },
		)?;

		self.last_updated = Some(when);

		Ok(self.value)
	}

	pub fn insert_elem(&mut self, id: ElemId, pv: Balance) -> DispatchResult {
		self.values
			.try_push((id, pv))
			.map_err(|_| DispatchError::Other("Max portfilio value reached"))?;

		Ok(self.value.ensure_add_assign(pv)?)
	}

	pub fn update_elem(&mut self, id: ElemId, new_pv: Balance) -> Result<bool, DispatchError> {
		let old_pv = self
			.values
			.iter_mut()
			.find(|(elem_id, _)| *elem_id == id)
			.map(|(_, value)| value)
			.ok_or(DispatchError::CannotLookup)?;

		let changed = match new_pv.cmp(old_pv) {
			Ordering::Greater => {
				let diff = new_pv.ensure_sub(*old_pv)?;
				self.value.ensure_add_assign(diff)?;
				true
			}
			Ordering::Less => {
				let diff = old_pv.ensure_sub(new_pv)?;
				self.value.ensure_sub_assign(diff)?;
				true
			}
			Ordering::Equal => false,
		};

		*old_pv = new_pv;

		Ok(changed)
	}
}

/// Information about how the portfolio valuation was updated
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum PortfolioValuationUpdateType {
	/// Portfolio Valuation was fully recomputed to an exact value
	Exact,
	/// Portfolio Valuation was updated inexactly based on loan status changes
	Inexact,
}
