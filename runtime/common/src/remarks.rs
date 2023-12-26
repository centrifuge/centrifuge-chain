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

use cfg_primitives::LoanId;
use frame_support::{dispatch::TypeInfo, parameter_types, BoundedVec};
use sp_runtime::codec::{Decode, Encode};
use sp_std::vec;

parameter_types! {
	pub const IpfsHashLength: u32 = 64;
	pub const MaxNamedRemark: u32 = 1024;
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode, TypeInfo)]
pub enum Remark {
	/// IPFS hash
	IpfsHash(BoundedVec<u8, IpfsHashLength>),

	/// UTF-8 encoded string
	Named(BoundedVec<u8, MaxNamedRemark>),

	/// Association with a loan
	Loan(LoanId),
}

impl Default for Remark {
	fn default() -> Self {
		Remark::Named(BoundedVec::try_from(vec![]).unwrap())
	}
}
