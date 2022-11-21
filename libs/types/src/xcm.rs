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

use cfg_primitives::Balance;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(
	Clone,
	Copy,
	Default,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct XcmMetadata {
	/// The fee charged for every second that an XCM message takes to execute.
	/// When `None`, the `default_per_second` will be used instead.
	pub fee_per_second: Option<Balance>,
}

pub mod consts {
	use frame_support::parameter_types;

	use super::*;

	// Pools-related constants
	pub mod pools {
		use super::*;

		parameter_types! {
			/// The max length in bytes allowed for a tranche token name
			#[derive(TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
			pub const MaxTrancheNameLengthBytes: u32 = 128;

			/// The max length in bytes allowed for a tranche token symbol
			#[derive(TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
			pub const MaxTrancheSymbolLengthBytes: u32 = 32;
		}
	}
}
