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

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;

/// [ISIN](https://en.wikipedia.org/wiki/International_Securities_Identification_Number) format.
pub type Isin = [u8; 12];

/// A representation of an oracle price identifier
#[derive(
	Encode,
	Decode,
	Clone,
	Copy,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	TypeInfo,
	RuntimeDebug,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum OracleKey {
	Isin(Isin),
}

#[cfg(feature = "runtime-benchmarks")]
impl From<u32> for OracleKey {
	fn from(value: u32) -> Self {
		// Any u32 value always fits into 12 bytes
		let isin = Isin::try_from(&(value as u128).to_be_bytes()[0..12]).unwrap();
		OracleKey::Isin(isin)
	}
}
