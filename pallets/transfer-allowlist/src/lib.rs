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
#![cfg_attr(not(feature = "std"), no_std)]

use cfg_primitives::AccountId;
pub use pallet::*;
use pallet_connectors::DomainAddress;
use sp_core::H160;
use sp_runtime::AccountId32;
use xcm::VersionedMultiLocation;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Location {
	Local(AccountId32),
	XCM(VersionedMultiLocation),
	Address(DomainAddress),
}

impl From<AccountId32> for Location {
	fn from(a: AccountId32) -> Location {
		Self::Local(a)
	}
}

impl From<VersionedMultiLocation> for Location {
	fn from(vml: VersionedMultiLocation) -> Location {
		Self::XCM(vml)
	}
}

impl From<DomainAddress> for Location {
	fn from(da: DomainAddress) -> Location {
		Self::Address(da)
	}
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::{DispatchError, DispatchResult},
		pallet_prelude::*,
		traits::Currency,
	};
	use frame_system::pallet_prelude::*;

	use super::*;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Deposit: Get<BalanceOf<Self>>;
		type Currency: Currency<Self::AccountId>;
	}
}

#[cfg(test)]
mod test {
	use cfg_primitives::AccountId;
	use hex::FromHex;
	use pallet_connectors::DomainAddress;
	use sp_core::H160;
	use xcm::{v1::MultiLocation, VersionedMultiLocation};

	use super::*;

	#[test]
	fn from_account_works() {
		let a: AccountId = AccountId::new([0; 32]);
		let l = Location::from(a.clone());
		assert_eq!(l, Location::Local(a))
	}

	#[test]
	fn from_xcm_address_works() {
		let xa = VersionedMultiLocation::V1(MultiLocation::default());
		let l = Location::from(xa.clone());
		assert_eq!(
			l,
			Location::XCM(VersionedMultiLocation::V1(MultiLocation::default()))
		)
	}
	#[test]
	fn from_domain_address_works() {
		let da = DomainAddress::EVM(
			1284,
			<[u8; 20]>::from_hex("1231231231231231231231231231231231231231").expect(""),
		);
		let l = Location::from(da.clone());
		assert_eq!(l, Location::Address(da))
	}
}
