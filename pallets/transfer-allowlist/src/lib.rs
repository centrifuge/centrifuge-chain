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

//! This module checks whether an account should be allowed to make a transfer to
//! a receiving location with a specific currency.
//! If there are no allowances specified, then the account is assumed to be allowed
//! to send to any location without restrictions.
//! However once an allowance for a sender to a specific recieving location and currency is made,
//! /then/ transfers from the sending account are restricted for that currency to:
//! - the account(s) for which allowances have been made
//! - the block range specified in the allowance

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use cfg_traits::TransferAllowance;
use cfg_types::locations::Location;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use core::fmt::Debug;

	use cfg_traits::ops::EnsureSub;
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{
		pallet_prelude::{DispatchResult, OptionQuery, StorageDoubleMap, StorageNMap, *},
		traits::{tokens::AssetId, Currency, ReservableCurrency},
		Twox64Concat,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use sp_runtime::{traits::AtLeast32BitUnsigned, Saturating};

	use super::*;

	pub type DepositBalanceOf<T> = <<T as Config>::ReserveCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	pub type AllowanceDetailsOf<T> = AllowanceDetails<<T as frame_system::Config>::BlockNumber>;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Id type of Currency transfer restrictions will be checked for
		type CurrencyId: AssetId
			+ Parameter
			+ Debug
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// Deposit Balance to reserve/release
		#[pallet::constant]
		type Deposit: Get<DepositBalanceOf<Self>>;

		/// Currency for Reserve/Unreserve with allowlist adding/removal,
		/// given that the allowlist will be in storage
		type ReserveCurrency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
	}

	//
	// Storage
	//
	/// Struct to define when a transfer should be allowed from
	/// the sender, receiver, and currency combination.
	/// Transfer allowed time set by range of block numbers
	/// Defaults to `allowed_at` starting at 0, and `blocked_at` ending at MAX block value
	/// as per `Default` impl.
	/// Current block must be between allowed at and blocked at
	/// for transfer to be approved if allowance for sender/currency/receiver present.
	#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
	pub struct AllowanceDetails<BlockNumber> {
		/// Specifies a block number after which transfers will be allowed
		/// for the sender & currency and destination location.
		/// This is by default set to 0 with the add allowance extrinsic,
		/// unless a delay is set, in which case it is set to the current block + delay.
		pub allowed_at: BlockNumber,
		/// Specifies a block number after-which transfers will be blocked
		/// for the sender & currency and destination location.
		/// This is by default set to `BlockNumber::Max()`, except when an allowance has been removed but not purged.
		/// In that case it is set to the current block + delay.
		/// if the allowance is later updated with the add allowance extrinsic, it is set back to max.
		pub blocked_at: BlockNumber,
	}

	impl<BlockNumber> Default for AllowanceDetails<BlockNumber>
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

	/// Storage item containing number of allowances set, and delay for a sending account and currency.
	/// Storage contains a tuple of the allowance count as `u64`, and the delay as `BlockNumber`--number of blocks that allow/block fields are delayed from current block.
	/// If a delay is set, but no allowances have been created, count will be set to 0.
	/// A double map is used here as we need to know whether there is a restriction set
	/// for the account and currency in the case where there is no allowance for destination location.
	/// Using an StorageNMap would not allow us to look up whether there was a restriction for the sending account and currency, given that:
	/// - we're checking whether there's an allowance specified for the receiver location
	///   - we would only find whether a restriction was set for the account in this caseif:
	///     - an allowance was specified for the receiving location, which would render blocked restrictions useless
	/// - we would otherwise need to store a vec of locations, which is problematic given that there isn't a set limit on receivers
	/// If a transfer restriction is in place, then a second lookup is done on
	/// AccountCurrencyAllowances to see if there is an allowance for the reciever
	/// This allows us to keep storage map vals to known/bounded sizes.
	#[pallet::storage]
	#[pallet::getter(fn get_account_currency_restriction_count_delay)]
	pub type AccountCurrencyTransferCountDelay<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		T::CurrencyId,
		(u64, Option<T::BlockNumber>),
		OptionQuery,
	>;

	/// Storage item for allowances specified for a sending account, currency type and receiving location
	#[pallet::storage]
	#[pallet::getter(fn get_account_currency_transfer_allowance)]
	pub type AccountCurrencyTransferAllowance<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Twox64Concat, T::AccountId>,
			NMapKey<Twox64Concat, T::CurrencyId>,
			NMapKey<Blake2_128Concat, Location>,
		),
		AllowanceDetails<T::BlockNumber>,
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
		DuplicateAllowance,
		/// CatchAll for Allowance Count arithmetic errors -- largely for coverage for errors that should be impossible
		AllowanceCountArithmeticError,
		/// No matching allowance for Location/Currency
		NoMatchingAllowance,
		/// No matching delay for the sending account and currency combination.
		/// Cannot delete a non-existant entry
		NoMatchingDelay,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event for successful creation of a transfer allowance
		TransferAllowanceCreated {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			receiver: Location,
			allowed_at: T::BlockNumber,
			blocked_at: T::BlockNumber,
		},
		/// Event for successful removal of transfer allowance perms
		TransferAllowanceRemoved {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			receiver: Location,
			allowed_at: T::BlockNumber,
			blocked_at: T::BlockNumber,
		},
		/// Event for successful removal of transfer allowance perms
		TransferAllowancePurged {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			receiver: Location,
		},
		/// Event for Allowance delay update
		TransferAllowanceDelaySet {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			delay: T::BlockNumber,
		},
		/// Event for Allowance delay removal
		TransferAllowanceDelayRemoval {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds a transfer allowance for a sending Account/Currency.
		/// Allowance either starts at the current block + the delay set for the account,
		/// if a delay is present.
		/// or block 0 if no delay is present.
		/// Important! Account/Currency sets with an allowance set are restricted to just the allowances added for the account -
		/// to have unrestricted transfers allowed for the sending Account and Currency, no allowances should be present.
		///
		/// Running this for an existing allowance generates a new allowance based on the current delay, or lack thereof
		#[pallet::call_index(0)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(2, 2).ref_time())]
		pub fn add_transfer_allowance(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			receiver: Location,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			let allowance_details = match Self::get_account_currency_restriction_count_delay(
				&account_id,
				currency_id,
			) {
				Some((_, Some(delay))) => AllowanceDetails {
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
				Self::increment_or_create_allowance_count(&account_id, &currency_id)?;
				T::ReserveCurrency::reserve(&account_id, T::Deposit::get())?;
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

		/// Restricts a transfer allowance for a sending account/currency/receiver location to:
		/// - either the current block + delay if a delay is set
		/// - or the current block if no delay is set
		#[pallet::call_index(1)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(2, 2).ref_time())]
		pub fn remove_transfer_allowance(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			receiver: Location,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			let blocked_at = match Self::get_account_currency_restriction_count_delay(
				&account_id,
				currency_id,
			) {
				Some((_, Some(delay))) => {
					<frame_system::Pallet<T>>::block_number().saturating_add(delay)
				}
				_ => <frame_system::Pallet<T>>::block_number(),
			};
			match <AccountCurrencyTransferAllowance<T>>::get((&account_id, &currency_id, &receiver))
			{
				Some(existing_allowance) => {
					let allowance_details = AllowanceDetails {
						blocked_at,
						..existing_allowance
					};
					<AccountCurrencyTransferAllowance<T>>::insert(
						(&account_id, &currency_id, &receiver),
						&allowance_details,
					);
					Self::deposit_event(Event::TransferAllowanceRemoved {
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

		/// Removes a transfer allowance for a sending account/currency and receiving location
		/// Decrements or removes the sending account/currency count.
		#[pallet::call_index(2)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1, 2).ref_time())]
		pub fn purge_transfer_allowance(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			receiver: Location,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			match <AccountCurrencyTransferAllowance<T>>::get((&account_id, &currency_id, &receiver))
			{
				Some(_) => {
					T::ReserveCurrency::unreserve(&account_id, T::Deposit::get());
					<AccountCurrencyTransferAllowance<T>>::remove((
						&account_id,
						&currency_id,
						&receiver,
					));
					Self::decrement_or_remove_allowance_count(&account_id, &currency_id)?;
					Self::deposit_event(Event::TransferAllowancePurged {
						sender_account_id: account_id,
						currency_id,
						receiver,
					});
					Ok(())
				}
				None => Err(DispatchError::from(Error::<T>::NoMatchingAllowance)),
			}
		}

		#[pallet::call_index(3)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(0, 1).ref_time())]
		/// Adds an account/currency delay
		/// Calling on an existing combination will update the existing delay value
		pub fn add_or_update_allowance_delay(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			delay: T::BlockNumber,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let (count, _) =
				Self::get_account_currency_restriction_count_delay(&account_id, &currency_id)
					.unwrap_or((0, Some(0u32.into())));
			<AccountCurrencyTransferCountDelay<T>>::insert(
				&account_id,
				&currency_id,
				(count, Some(delay.clone())),
			);
			Self::deposit_event(Event::TransferAllowanceDelaySet {
				sender_account_id: account_id,
				currency_id,
				delay,
			});
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(0, 1).ref_time())]
		/// Removes an existing sending account/currency delay
		pub fn remove_allowance_delay(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			match Self::get_account_currency_restriction_count_delay(&account_id, &currency_id) {
				Some((count, _)) if count == 0 => {
					<AccountCurrencyTransferCountDelay<T>>::remove(&account_id, &currency_id);
					Self::deposit_event(Event::TransferAllowanceDelayRemoval {
						sender_account_id: account_id,
						currency_id,
					});
					Ok(())
				}

				Some((count, _)) => {
					<AccountCurrencyTransferCountDelay<T>>::insert(
						&account_id,
						&currency_id,
						&(count, None),
					);
					Self::deposit_event(Event::TransferAllowanceDelayRemoval {
						sender_account_id: account_id,
						currency_id,
					});
					Ok(())
				}
				None => {
					Self::deposit_event(Event::TransferAllowanceDelayRemoval {
						sender_account_id: account_id,
						currency_id,
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
			match Self::get_account_currency_restriction_count_delay(account_id, currency_id) {
				Some((allowance_count, delay)) => {
					let new_allowance_count = allowance_count
						.checked_add(1)
						.ok_or(Error::<T>::AllowanceCountOverflow)?;
					<AccountCurrencyTransferCountDelay<T>>::insert(
						account_id,
						currency_id,
						(new_allowance_count, delay),
					);
					Ok(())
				}
				_ => {
					<AccountCurrencyTransferCountDelay<T>>::insert(
						account_id,
						currency_id,
						(1, None::<T::BlockNumber>),
					);
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
			match Self::get_account_currency_restriction_count_delay(account_id, currency_id) {
				Some((allowance_count, None)) if allowance_count <= 1 => {
					<AccountCurrencyTransferCountDelay<T>>::remove(account_id, currency_id);
					Ok(())
				}
				Some((allowance_count, delay)) if allowance_count <= 1 => {
					<AccountCurrencyTransferCountDelay<T>>::insert(
						account_id,
						currency_id,
						(0, delay),
					);
					Ok(())
				}
				Some((allowance_count, delay)) => {
					// check in this case should not ever be needed
					let new_allowance_count = allowance_count.ensure_sub(1)?;
					<AccountCurrencyTransferCountDelay<T>>::insert(
						account_id,
						currency_id,
						(new_allowance_count, delay),
					);
					Ok(())
				}
				_ => Err(DispatchError::from(Error::<T>::NoAllowancesSet)),
			}
		}
	}

	impl<T: Config> TransferAllowance<T::AccountId> for Pallet<T> {
		type CurrencyId = T::CurrencyId;
		type Location = Location;

		/// This checks to see if a transfer from an account and currency should be allowed to a given location.
		/// If there are no allowances defined for the sending account and currency, then the transfer is allowed.
		/// If there is an allowance for the sending account and currency,
		/// but the destination does not have an allowance added then the transfer is not allowed.
		/// If there is an allowance for the sending account and currency,
		/// and there's an allowance present:
		/// then we check whether the current block is between the `allowed_at` and `blocked_at` blocks in the allowance.
		fn allowance(
			send: T::AccountId,
			receive: Self::Location,
			currency: T::CurrencyId,
		) -> Result<bool, DispatchError> {
			match Self::get_account_currency_restriction_count_delay(&send, &currency) {
				Some((count, _)) if count > 0 => {
					let current_block = <frame_system::Pallet<T>>::block_number();
					match <AccountCurrencyTransferAllowance<T>>::get((&send, &currency, receive)) {
						Some(AllowanceDetails {
							allowed_at,
							blocked_at,
						}) if current_block >= allowed_at && current_block < blocked_at => Ok(true),
						_ => Ok(false),
					}
				}
				// In this case no allowances are set for the sending account & currency,
				// therefore no restrictions should be in place.
				_ => Ok(true),
			}
		}
	}
}
