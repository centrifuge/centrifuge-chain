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
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::{DispatchError, DispatchResult};
pub use pallet::*;
use pallet_connectors::DomainAddress;
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::AccountId32;
use xcm::v1::MultiLocation;

#[derive(Clone, Encode, Debug, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
pub enum Location {
	Local(AccountId32),
	// unfortunately VersionedMultiLocation does not implmenent MaxEncodedLen, and
	// both are foreign, and therefore can't be implemented here.
	// may move back to new type off VersionedMultiLocation w/ MaxEncodedLen implemented
	// if it looks like nothing will be Location enum outside of pallet
	XCMV1(MultiLocation),
	Address(DomainAddress),
}

impl From<AccountId32> for Location {
	fn from(a: AccountId32) -> Location {
		Self::Local(a)
	}
}

// using
impl From<MultiLocation> for Location {
	fn from(ml: MultiLocation) -> Location {
		Self::XCMV1(ml)
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
		pallet_prelude::{
			DispatchResult, OptionQuery, StorageDoubleMap, StorageNMap, ValueQuery, *,
		},
		Twox64Concat,
	};
	use frame_system::pallet_prelude::*;
	use xcm::{v1::MultiLocation, VersionedMultiLocation};

	use super::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);
	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	pub type CurrencyIdOf<T> = <T as Config>::CurrencyId;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The currency-id type of this pallet
		type CurrencyId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;
	}

	#[derive(
		Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, MaxEncodedLen, TypeInfo,
	)]
	pub struct AllowanceDetails<BlockNumberOf> {
		allowed_at: BlockNumberOf, // Defaults to 0
		blocked_at: BlockNumberOf, // Defaults to BlockNumber::MAX
	}

	trait TransferAllowance<AccountId, Location> {
		type CurrencyId;
		fn allowance(
			send: AccountId,
			recieve: Location,
			currency: Self::CurrencyId,
		) -> DispatchResult;
	}

	// impl<T: Config> TransferAllowance<Self::AccountId, Self::AccountId> for Pallet<T> {
	// 	type CurrencyId = Self::CurrencyId;
	//   fn allowance(send: Self::AccountId, recieve: VersionedMultiLocation, currency: Self::Currency) -> DispatchResult {
	//       match <AccountCurrencyAllowances<T>>::get(send, currency) {
	//           Some(true)
	//       }
	//   }
	// }

	#[pallet::type_value]
	pub fn DefaultHasRestrictions<T: Config>() -> bool {
		false
	}
	#[pallet::storage]
	pub type AccountCurrencyTransferRestriction<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		AccountIdOf<T>,
		Twox64Concat,
		CurrencyIdOf<T>,
		bool,
		ValueQuery,
		DefaultHasRestrictions<T>,
	>;

	#[pallet::storage]
	pub type AccountCurrencyAllowances<T> = StorageNMap<
		_,
		(
			NMapKey<Twox64Concat, AccountIdOf<T>>,
			NMapKey<Twox64Concat, CurrencyIdOf<T>>,
			NMapKey<Blake2_128Concat, Location>,
		),
		AllowanceDetails<BlockNumberOf<T>>,
		OptionQuery,
	>;
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
		let xa = MultiLocation::default();
		let l = Location::from(xa.clone());
		assert_eq!(l, Location::XCMV1(MultiLocation::default()))
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
