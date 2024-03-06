// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

use cfg_traits::Seconds;
use frame_support::pallet_prelude::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::{One, Saturating};

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EpochState<EpochId> {
	/// Current epoch that is ongoing.
	pub current: EpochId,
	/// Time when the last epoch was closed.
	pub last_closed: Seconds,
	/// Last epoch that was executed.
	pub last_executed: EpochId,
}

impl<EpochId: Ord + Saturating + Copy + One> EpochState<EpochId> {
	/// ```text
	///                      submission_period
	///                    <------------------->
	/// -------------------|-------------------|-----------------------
	///  current = i + 1   |  current = i + 2  |
	///  last_executed = i |                   | last_executed = i + 1
	/// -------------------|-------------------|-----------------------
	///                 close_epoch()     execute_epoch()
	/// ```
	pub fn is_submission_period(&self) -> bool {
		self.last_executed.saturating_add(One::one()) < self.current
	}
}
