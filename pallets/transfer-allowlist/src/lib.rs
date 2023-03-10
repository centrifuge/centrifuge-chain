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
use sp_runtime::{traits::IdentifyAccount, AccountId32};
use xcm::v1::MultiLocation;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

/// Location types for destinations that can receive restricted transfers
#[derive(Clone, Encode, Debug, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub enum Location<T: Config> {
	Local(AccountIdOf<T>),
	// unfortunately VersionedMultiLocation does not implmenent MaxEncodedLen, and
	// both are foreign, and therefore can't be implemented here.
	// may move back to new type off VersionedMultiLocation w/ MaxEncodedLen implemented
	// if it looks like nothing will be Location enum outside of pallet
	XCMV1(MultiLocation),
	Address(DomainAddress),
}

// impl<T: Config + frame_system::Config<AccountId = T>> From<T> for Location<T>
// where
// 	T::AccountId: IdentifyAccount,
// {
// 	fn from(a: AccountIdOf<T>) -> Self {
// 		Self::Local(a)
// 	}
// }

impl<T: Config> From<MultiLocation> for Location<T> {
	fn from(ml: MultiLocation) -> Self {
		Self::XCMV1(ml)
	}
}

impl<T: Config> From<DomainAddress> for Location<T> {
	fn from(da: DomainAddress) -> Self {
		Self::Address(da)
	}
}

/// Trait to determine whether a sending account and currency have a restriction,
/// and if so is there an allowance for the reciever location.
trait TransferAllowance<AccountId, Location> {
	type CurrencyId;
	fn allowance(
		send: AccountId,
		recieve: Location,
		currency: Self::CurrencyId,
	) -> Result<bool, DispatchError>;
}

impl<T: Config> TransferAllowance<T::AccountId, T::AccountId> for Pallet<T>
where
	T::AccountId: IdentifyAccount,
{
	type CurrencyId = T::CurrencyId;

	fn allowance(
		send: T::AccountId,
		recieve: T::AccountId,
		currency: T::CurrencyId,
	) -> Result<bool, DispatchError> {
		match <AccountCurrencyTransferRestriction<T>>::get(&send, &currency) {
			Some(count) if count > 0 => {
				let current_block = <frame_system::Pallet<T>>::block_number();
				match <AccountCurrencyTransferAllowance<T>>::get((
					&send,
					&currency,
					Location::Local(recieve),
				)) {
					Some(AllowanceDetails {
						allowed_at: allowed_at,
						blocked_at: blocked_at,
					}) if current_block >= allowed_at && current_block < blocked_at => Ok(true),
					_ => Ok(false),
				}
			}
			_ => Ok(true),
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::{
			DispatchResult, OptionQuery, StorageDoubleMap, StorageNMap, ValueQuery, *,
		},
		traits::{tokens::AssetId, GenesisBuild},
		transactional, Twox64Concat,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use sp_runtime::traits::AtLeast32BitUnsigned;
	use xcm::{v1::MultiLocation, VersionedMultiLocation};

	use super::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type CurrencyId: AssetId
			+ Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;
	}

	pub type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	pub type CurrencyIdOf<T> = <T as Config>::CurrencyId;

	// --------------------------
	//          Storage
	// --------------------------
	pub type AllowanceDetailsOf<T> = AllowanceDetails<BlockNumberOf<T>>;
	#[derive(
		Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, MaxEncodedLen, TypeInfo,
	)]

	/// Struct to define when a transfer should be allowed from
	/// the sender, receiver, and currency combination.
	/// Transfer allowed time set by range of block numbers
	/// Defaults to starting at 0, and ending at MAX block value
	/// as per default.
	pub struct AllowanceDetails<BlockNumber> {
		pub allowed_at: BlockNumber,
		pub blocked_at: BlockNumber,
	}

	impl<BlockNumber> AllowanceDetails<BlockNumber>
	where
		BlockNumber: AtLeast32BitUnsigned,
	{
		fn default() -> Self {
			Self {
				allowed_at: BlockNumber::zero(),
				blocked_at: BlockNumber::max_value(),
			}
		}
	}

	/// Storage item for whether a sending account and currency have restrictions set
	/// a double map is used here as we need to know whether there is a restriction set
	/// for the account and currency.
	/// Using an StorageNMap would not allow us to look up whether there was a restriction for the sending account and currency, given that:
	/// - we're checking whether there's an allowance specified for the receiver location
	///   - we would only find whether a restriction was set for the account in this caseif:
	///     - an allowance was specified for the receiving location, which would render blocked restrictions useless
	/// - we would otherwise need to store a vec of locations, which is problematic given that there isn't a set limit on receivers
	/// If a transfer restriction is in place, then a second lookup is done on
	/// AccountCurrencyAllowances to see if there is an allowance for the reciever
	/// This allows us to keep storage map vals to known/bounded sizes.
	#[pallet::storage]
	pub type AccountCurrencyTransferRestriction<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		AccountIdOf<T>,
		Twox64Concat,
		CurrencyIdOf<T>,
		u64,
		OptionQuery,
	>;

	/// Storage item for allowances specified for a sending account, currency type and drecieving location
	#[pallet::storage]
	pub type AccountCurrencyTransferAllowance<T> = StorageNMap<
		_,
		(
			NMapKey<Twox64Concat, AccountIdOf<T>>,
			NMapKey<Twox64Concat, CurrencyIdOf<T>>,
			NMapKey<Blake2_128Concat, Location<T>>,
		),
		AllowanceDetails<BlockNumberOf<T>>,
		OptionQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[transactional]
		#[pallet::call_index(0)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1, 2).ref_time())]
		pub fn add_sender_account_transfer_restriction(
			origin: OriginFor<T>,
			currency: CurrencyIdOf<T>,
			receiver: Location<T>,
		) -> DispatchResult {
			Ok(())
		}
	}
}

#[cfg(test)]
mod test {}
