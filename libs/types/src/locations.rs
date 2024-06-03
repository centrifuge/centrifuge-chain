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

use cfg_primitives::AccountId;
use frame_support::RuntimeDebugNoBound;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::crypto::AccountId32;
// Please note that if this version change,
// a migration could be required in those places where
// RestrictedTransferLocation is stored
use staging_xcm::v4::Location;

use crate::domain_address::DomainAddress;
/// Location types for destinations that can receive restricted transfers
#[derive(Clone, RuntimeDebugNoBound, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
pub enum RestrictedTransferLocation {
	/// Local chain account sending destination.
	Local(AccountId),
	/// XCM Location sending destinations.
	Xcm(Location),
	/// DomainAddress sending location from a liquidity pools' instance
	Address(DomainAddress),
}

impl From<AccountId32> for RestrictedTransferLocation {
	fn from(value: AccountId32) -> Self {
		Self::Local(value)
	}
}
