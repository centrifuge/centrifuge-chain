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

/// This module checks whether an account should be allowed to make a transfer to
/// a recieving location with a specific currency.
/// If there are no allowances specified, then the account is assumed to be allowed
/// to send to any location without restrictions.
/// However once an allowance for a sender to a specific recieving location and currency is made,
/// /then/ transfers from the sending account are restricted for that currency to:
/// - the account(s) for which allowances have been made
/// - the block range specified in the allowance
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	RuntimeDebugNoBound,
};
pub use pallet::*;
use pallet_connectors::DomainAddress;
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::traits::IdentifyAccount;
use xcm::v1::MultiLocation;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// AccountId type for runtime used in pallet.
pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

/// Location types for destinations that can receive restricted transfers
#[derive(Clone, RuntimeDebugNoBound, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub enum Location<T: Config> {
	/// Local chain account sending destination.
	Local(AccountIdOf<T>),
	/// XCM V1 MultiLocation sending destination.
	/// Unfortunately VersionedMultiLocation does not implmenent MaxEncodedLen, and
	/// both are foreign, and therefore can't be implemented here.
	// May move back to new type off VersionedMultiLocation w/ MaxEncodedLen implemented
	// if it looks like nothing will be Location enum outside of pallet.
	XCMV1(MultiLocation),
	/// DomainAddress sending location from connectors
	Address(DomainAddress),
}

/// Helper struct for account id  `from` impl due to generic impl conflict
/// See:
/// https://doc.rust-lang.org/error_codes/E0119.html and
/// https://github.com/rust-lang/rust/issues/50133#issuecomment-64690839
pub struct AccountWrapper<T: Config>(AccountIdOf<T>);

impl<T: Config> From<AccountWrapper<T>> for Location<T> {
	fn from(a: AccountWrapper<T>) -> Self {
		Self::Local(a.0)
	}
}

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
pub trait TransferAllowance<AccountId, Location> {
	type CurrencyId;
	/// Determines whether the `send` account is allowed to make a transfer to the  `recieve` loocation with `currency` type currency.
	/// Returns result wrapped bool for whether allowance is allowed.
	fn allowance(
		send: AccountId,
		recieve: Location,
		currency: Self::CurrencyId,
	) -> Result<bool, DispatchError>;
}

#[frame_support::pallet]
pub mod pallet {
	use core::fmt::Debug;

	use frame_support::{
		pallet_prelude::{
			DispatchResult, OptionQuery, StorageDoubleMap, StorageNMap, ValueQuery, *,
		},
		traits::{tokens::AssetId, GenesisBuild},
		transactional, Twox64Concat,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use sp_runtime::{traits::AtLeast32BitUnsigned, Saturating};
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
			+ Debug
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

	//
	// Storage
	//
	pub type AllowanceDetailsOf<T> = AllowanceDetails<BlockNumberOf<T>>;

	/// Struct to define when a transfer should be allowed from
	/// the sender, receiver, and currency combination.
	/// Transfer allowed time set by range of block numbers
	/// Defaults to starting at 0, and ending at MAX block value
	/// as per default.
	#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, Default, MaxEncodedLen, TypeInfo)]
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
	#[pallet::getter(fn sender_currency_restriction_set)]
	pub type AccountCurrencyTransferRestriction<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		AccountIdOf<T>,
		Twox64Concat,
		CurrencyIdOf<T>,
		u64,
		OptionQuery,
	>;

	/// Storage item for allowances specified for a sending account, currency type and recieving location
	#[pallet::storage]
	#[pallet::getter(fn sender_currency_reciever_allowance)]
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
	/// Storage item for Allowance delays for a sending account/currency
	#[pallet::storage]
	#[pallet::getter(fn sender_currency_delay)]
	pub type AccountCurrencyDelay<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		AccountIdOf<T>,
		Twox64Concat,
		CurrencyIdOf<T>,
		BlockNumberOf<T>,
		OptionQuery,
	>;

	//
	// Pallet Errors and Events
	//
	#[pallet::error]
	pub enum Error<T> {
		/// Number of allowances for a sending Account/Currency set at max allowance count storage type val (currently u64::MAX)
		AllowanceCountOverflow,
		/// An operation expecting one or more allowances for a sending Account/Currency set, where none present
		NoAllowancesSet,
		/// Attempted to create allowance for existing Sending Account, Currency, and Receiver combination
		ConflictingAllowanceSet,
		/// CatchAll for Allowance Count arithmetic errors -- largely for coverage for errors that should be impossible
		AllowanceCountArithmeticError,
		/// No matching allowance for Location/Currency
		NoMatchingAllowance,
		/// No matching delay
		NoMatchingDelay,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event for successful creation of a transfer allowance
		TransferAllowanceCreated {
			sender_account_id: AccountIdOf<T>,
			currency_id: CurrencyIdOf<T>,
			receiver: Location<T>,
			allowed_at: BlockNumberOf<T>,
			blocked_at: BlockNumberOf<T>,
		},
		/// Event for successful removal of a tra
		TransferAllowanceRemoved {
			sender_account_id: AccountIdOf<T>,
			currency_id: CurrencyIdOf<T>,
			receiver: Location<T>,
		},
		/// Event for Allowance delay update
		TransferAllowanceDelaySet {
			sender_account_id: AccountIdOf<T>,
			currency_id: CurrencyIdOf<T>,
			delay: BlockNumberOf<T>,
		},
		/// Event for Allowance delay removal
		TransferAllowanceDelayRemoval {
			sender_account_id: AccountIdOf<T>,
			currency_id: CurrencyIdOf<T>,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[transactional]
		#[pallet::call_index(0)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(2, 2).ref_time())]
		/// Adds a transfer allowance for a sending Account/Currency.
		/// Allowance either starts at the current block + the delay set for the account,
		/// if a delay is present.
		/// or block 0 if no delay is present.
		/// Important! Account/Currency sets with an allowance set are restricted to just the allowances added for the account -
		/// to have unrestricted transfers allowed for the sending Account and Currency, no allowances should be present.
		pub fn add_transfer_allowance(
			origin: OriginFor<T>,
			currency_id: CurrencyIdOf<T>,
			receiver: Location<T>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			let allowance_details = match Self::sender_currency_delay(&account_id, currency_id) {
				Some(delay) => AllowanceDetails {
					allowed_at: <frame_system::Pallet<T>>::block_number().saturating_add(delay),
					..AllowanceDetails::default()
				},
				_ => AllowanceDetails::default(),
			};
			if !<AccountCurrencyTransferAllowance<T>>::contains_key((
				&account_id,
				&currency_id,
				&receiver,
			)) {
				Self::increment_or_create_allowance_count(&account_id, &currency_id)?
			};
			<AccountCurrencyTransferAllowance<T>>::insert(
				(&account_id, &currency_id, &receiver),
				&allowance_details,
			);

			Self::deposit_event(Event::TransferAllowanceCreated {
				sender_account_id: account_id,
				currency_id,
				receiver,
				allowed_at: allowance_details.allowed_at,
				blocked_at: allowance_details.blocked_at,
			});
			Ok(())
		}

		#[transactional]
		#[pallet::call_index(1)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(2, 2).ref_time())]
		pub fn remove_transfer_allowance(
			origin: OriginFor<T>,
			currency_id: CurrencyIdOf<T>,
			receiver: Location<T>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			let blocked_at = match Self::sender_currency_delay(&account_id, currency_id) {
				Some(delay) => <frame_system::Pallet<T>>::block_number().saturating_add(delay),
				_ => <frame_system::Pallet<T>>::block_number(),
			};
			match <AccountCurrencyTransferAllowance<T>>::get((&account_id, &currency_id, &receiver))
			{
				Some(existing_allowance) => {
					let allowance_details = AllowanceDetails {
						blocked_at: blocked_at.clone(),
						..existing_allowance
					};
					<AccountCurrencyTransferAllowance<T>>::insert(
						(&account_id, &currency_id, &receiver),
						&allowance_details,
					);
					Self::decrement_or_remove_allowance_count(&account_id, &currency_id)?;
					Self::deposit_event(Event::TransferAllowanceCreated {
						sender_account_id: account_id,
						currency_id,
						receiver,
						allowed_at: allowance_details.allowed_at,
						blocked_at: allowance_details.blocked_at,
					});
					Ok(())
				}
				None => Err(DispatchError::from(Error::<T>::NoMatchingAllowance)),
			}
		}

		#[transactional]
		#[pallet::call_index(2)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1, 2).ref_time())]
		pub fn purge_transfer_allowance(
			origin: OriginFor<T>,
			currency_id: CurrencyIdOf<T>,
			receiver: Location<T>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			match <AccountCurrencyTransferAllowance<T>>::get((&account_id, &currency_id, &receiver))
			{
				Some(_) => {
					<AccountCurrencyTransferAllowance<T>>::remove((
						&account_id,
						&currency_id,
						&receiver,
					));
					Self::decrement_or_remove_allowance_count(&account_id, &currency_id)?;
					Self::deposit_event(Event::TransferAllowanceRemoved {
						sender_account_id: account_id,
						currency_id,
						receiver,
					});
					Ok(())
				}
				None => Err(DispatchError::from(Error::<T>::NoMatchingAllowance)),
			}
		}

		#[transactional]
		#[pallet::call_index(3)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(0, 1).ref_time())]
		pub fn add_or_update_allowance_delay(
			origin: OriginFor<T>,
			currency_id: CurrencyIdOf<T>,
			delay: BlockNumberOf<T>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			<AccountCurrencyDelay<T>>::insert(&account_id, &currency_id, &delay);
			Self::deposit_event(Event::TransferAllowanceDelaySet {
				sender_account_id: account_id,
				currency_id: currency_id,
				delay: delay,
			});
			Ok(())
		}

		#[transactional]
		#[pallet::call_index(4)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(0, 1).ref_time())]
		pub fn remove_allowance_delay(
			origin: OriginFor<T>,
			currency_id: CurrencyIdOf<T>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			match <AccountCurrencyDelay<T>>::get(&account_id, &currency_id) {
				Some(_) => {
					<AccountCurrencyDelay<T>>::remove(&account_id, &currency_id);
					Self::deposit_event(Event::TransferAllowanceDelayRemoval {
						sender_account_id: account_id,
						currency_id: currency_id,
					});
					Ok(())
				}
				None => {
					Self::deposit_event(Event::TransferAllowanceDelayRemoval {
						sender_account_id: account_id,
						currency_id: currency_id,
					});
					Err(DispatchError::from(Error::<T>::NoMatchingDelay))
				}
			}
		}
	}

	impl<T: Config> Pallet<T> {
		/// Increments number of allowances present for a sending account/currency set.
		/// If no allowances set, an entry with 1 added, if entry already present, it is then incremented.
		pub fn increment_or_create_allowance_count(
			account_id: &T::AccountId,
			currency_id: &T::CurrencyId,
		) -> DispatchResult {
			// not using try_mutate here as we're not sure if key exits, and we're already doing a some value check on result of exists query check
			match (<AccountCurrencyTransferRestriction<T>>::get(account_id, currency_id)) {
				Some(allowance_count) if allowance_count > 0 => {
					let new_allowance_count = allowance_count
						.checked_add(1)
						.ok_or(Error::<T>::AllowanceCountOverflow)?;
					<AccountCurrencyTransferRestriction<T>>::insert(
						account_id,
						currency_id,
						new_allowance_count,
					);
					Ok(())
				}
				Some(_) => Err(DispatchError::from(
					Error::<T>::AllowanceCountArithmeticError,
				)),
				_ => {
					<AccountCurrencyTransferRestriction<T>>::insert(account_id, currency_id, 1);
					Ok(())
				}
			}
		}

		/// Decrements the number of allowances tracked for a sending account/currency set.
		/// If the allowance count is currently 1, then it removes the entry
		/// If greater than 1, then decremented.
		/// If no entry present, NoAllowancesSet error returned.
		pub fn decrement_or_remove_allowance_count(
			account_id: &T::AccountId,
			currency_id: &T::CurrencyId,
		) -> DispatchResult {
			// not using try_mutate here as we're not sure if key exits, and we're already doing a some value check on result of exists query check
			match (<AccountCurrencyTransferRestriction<T>>::get(account_id, currency_id)) {
				Some(allowance_count) if allowance_count <= 1 => {
					<AccountCurrencyTransferRestriction<T>>::remove(account_id, currency_id);
					Ok(())
				}
				Some(allowance_count) => {
					// check in this case should not ever be needed
					let new_allowance_count = allowance_count
						.checked_sub(1)
						.ok_or(Error::<T>::AllowanceCountArithmeticError)?;
					<AccountCurrencyTransferRestriction<T>>::insert(
						account_id,
						currency_id,
						new_allowance_count,
					);
					Ok(())
				}
				_ => Err(DispatchError::from(Error::<T>::NoAllowancesSet)),
			}
		}
	}

	impl<T: Config> TransferAllowance<T::AccountId, T::AccountId> for Pallet<T> {
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
}

#[cfg(test)]
mod test {}
