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

//! This module checks whether an account should be allowed to make a transfer
//! to a receiving location with a specific currency.
//! If there are no allowances specified, then the account is assumed to be
//! allowed to send to any location without restrictions.
//! However once an allowance for a sender to a specific recieving location and
//! currency is made, /then/ transfers from the sending account are restricted
//! for that currency to:
//! - the account(s) for which allowances have been made
//! - the block range specified in the allowance

#[cfg(test)]
pub(crate) mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

pub use cfg_traits::TransferAllowance;
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use core::fmt::Debug;

	use cfg_traits::fees::Fees;
	use codec::{Decode, Encode, EncodeLike, MaxEncodedLen};
	use frame_support::{
		pallet_prelude::{DispatchResult, Member, OptionQuery, StorageDoubleMap, StorageNMap, *},
		traits::{tokens::AssetId, Currency, ReservableCurrency},
		Twox64Concat,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use scale_info::TypeInfo;
	use sp_runtime::{
		traits::{AtLeast32BitUnsigned, EnsureAdd, EnsureSub},
		Saturating,
	};

	use super::*;

	/// Balance type for the reserve/deposit made when creating an Allowance
	pub type DepositBalanceOf<T> = <<T as Config>::ReserveCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	/// AllowanceDetails where `BlockNumber` is of type BlockNumberFor<T>
	pub type AllowanceDetailsOf<T> = AllowanceDetails<<T as frame_system::Config>::BlockNumber>;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]

	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Id type of Currency transfer restrictions will be checked for
		type CurrencyId: AssetId
			+ Parameter
			+ Debug
			+ Default
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// Currency for Reserve/Unreserve with allowlist adding/removal,
		/// given that the allowlist will be in storage
		type ReserveCurrency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		/// Fee handler for the reserve/unreserve
		/// Currently just stores the amounts, will be extended to handle
		/// reserve/unreserve as well in future
		type Fees: Fees<
			AccountId = <Self as frame_system::Config>::AccountId,
			Balance = DepositBalanceOf<Self>,
		>;

		/// Fee Key used to find amount for allowance reserve/unreserve
		type AllowanceFeeKey: Get<<Self::Fees as Fees>::FeeKey>;

		/// Type containing the locations a transfer can be sent to.
		type Location: Member
			+ Debug
			+ Eq
			+ PartialEq
			+ TypeInfo
			+ Encode
			+ EncodeLike
			+ Decode
			+ MaxEncodedLen;

		/// Type for pallet weights
		type WeightInfo: WeightInfo;
	}

	//
	// Storage
	//
	/// Struct to define when a transfer should be allowed from
	/// the sender, receiver, and currency combination.
	/// Transfer allowed time set by range of block numbers
	/// Defaults to `allowed_at` starting at 0, and `blocked_at` ending at MAX
	/// block value as per `Default` impl.
	/// Current block must be between allowed at and blocked at
	/// for transfer to be approved if allowance for sender/currency/receiver
	/// present.
	#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
	pub struct AllowanceDetails<BlockNumber> {
		/// Specifies a block number after which transfers will be allowed
		/// for the sender & currency and destination location.
		/// This is by default set to 0 with the add allowance extrinsic,
		/// unless a delay is set, in which case it is set to the current block
		/// + delay.
		pub allowed_at: BlockNumber,
		/// Specifies a block number after-which transfers will be blocked
		/// for the sender & currency and destination location.
		/// This is by default set to `BlockNumber::Max()`, except when an
		/// allowance has been removed but not purged. In that case it is set to
		/// the current block + delay. if the allowance is later updated with
		/// the add allowance extrinsic, it is set back to max.
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

	/// Metadata values used to track and manage Allowances for a sending
	/// Account/Currency combination. contains the number of allowances/presence
	/// of existing allowances for said combination, as well as whether a delay
	/// is set for the allowance to take effect, and if--and then when--a delay
	/// is modifiable.
	#[derive(Clone, Copy, Debug, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
	pub struct AllowanceMetadata<BlockNumber> {
		pub(super) allowance_count: u64,
		pub(super) current_delay: Option<BlockNumber>,
		pub(super) once_modifiable_after: Option<BlockNumber>,
	}

	impl<BlockNumber> Default for AllowanceMetadata<BlockNumber>
	where
		BlockNumber: AtLeast32BitUnsigned,
	{
		fn default() -> Self {
			Self {
				allowance_count: 1u64,
				current_delay: None,
				once_modifiable_after: None,
			}
		}
	}
	/// Storage item containing number of allowances set, delay for sending
	/// account/currency, and block number delay is modifiable at. Contains an
	/// instance of AllowanceMetadata with allowance count as `u64`,
	/// current_delay as `Option<BlockNumberFor<T>>`, and modifiable_at as
	/// `Option<BlockNumberFor<T>>`. If a delay is set, but no allowances have
	/// been created, `allowance_count` will be set to `0`. A double map is used
	/// here as we need to know whether there is a restriction set for the
	/// account and currency in the case where there is no allowance for
	/// destination location. Using an StorageNMap would not allow us to look up
	/// whether there was a restriction for the sending account and currency,
	/// given that:
	/// - we're checking whether there's an allowance specified for the receiver
	///   location
	///   - we would only find whether a restriction was set for the account in
	///     this case if:
	///     - an allowance was specified for the receiving location, which would
	///       render blocked restrictions useless
	/// - we would otherwise need to store a vec of locations, which is
	///   problematic given that there isn't a set limit on receivers
	/// If a transfer restriction is in place, then a second lookup is done on
	/// AccountCurrencyAllowances to see if there is an allowance for the
	/// receiver This allows us to keep storage map vals to known/bounded sizes.
	#[pallet::storage]
	#[pallet::getter(fn get_account_currency_restriction_count_delay)]
	pub type AccountCurrencyTransferCountDelay<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		T::CurrencyId,
		AllowanceMetadata<BlockNumberFor<T>>,
		OptionQuery,
	>;

	/// Storage item for allowances specified for a sending account, currency
	/// type and receiving location
	#[pallet::storage]
	#[pallet::getter(fn get_account_currency_transfer_allowance)]
	pub type AccountCurrencyTransferAllowance<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Twox64Concat, T::AccountId>,
			NMapKey<Twox64Concat, T::CurrencyId>,
			NMapKey<Blake2_128Concat, T::Location>,
		),
		AllowanceDetails<BlockNumberFor<T>>,
		OptionQuery,
	>;

	//
	// Pallet Errors and Events
	//
	#[pallet::error]
	pub enum Error<T> {
		/// An operation expecting one or more allowances for a sending
		/// Account/Currency set, where none present
		NoAllowancesSet,
		/// Attempted to create allowance for existing Sending Account,
		/// Currency, and Receiver combination
		DuplicateAllowance,
		/// No matching allowance for Location/Currency
		NoMatchingAllowance,
		/// No matching delay for the sending account and currency combination.
		/// Cannot delete a non-existant entry
		NoMatchingDelay,
		/// Delay already exists
		DuplicateDelay,
		/// Delay has not been set to modified, or delay at which modification
		/// has been set has not been reached.
		DelayUnmodifiable,
		/// Attempted to clear active allowance
		AllowanceHasNotExpired,
		/// Transfer from sending account and currency not allowed to
		/// destination
		NoAllowanceForDestination,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event for successful creation of a transfer allowance
		TransferAllowanceCreated {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			receiver: T::Location,
			allowed_at: BlockNumberFor<T>,
			blocked_at: BlockNumberFor<T>,
		},
		/// Event for successful removal of transfer allowance perms
		TransferAllowanceRemoved {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			receiver: T::Location,
			allowed_at: BlockNumberFor<T>,
			blocked_at: BlockNumberFor<T>,
		},
		/// Event for successful removal of transfer allowance perms
		TransferAllowancePurged {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			receiver: T::Location,
		},
		/// Event for Allowance delay creation
		TransferAllowanceDelayAdd {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			delay: BlockNumberFor<T>,
		},
		/// Event for Allowance delay update
		TransferAllowanceDelayUpdate {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			delay: BlockNumberFor<T>,
		},
		/// Event for Allowance delay future modification allowed
		ToggleTransferAllowanceDelayFutureModifiable {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
			modifiable_once_after: Option<BlockNumberFor<T>>,
		},
		/// Event for Allowance delay removal
		TransferAllowanceDelayPurge {
			sender_account_id: T::AccountId,
			currency_id: T::CurrencyId,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adds a transfer allowance for a sending Account/Currency.
		/// Allowance either starts at the current block + the delay set for the
		/// account, if a delay is present.
		/// or block 0 if no delay is present.
		/// Important! Account/Currency sets with an allowance set are
		/// restricted to just the allowances added for the account -
		/// to have unrestricted transfers allowed for the sending Account and
		/// Currency, no allowances should be present.
		///
		/// Running this for an existing allowance generates a new allowance
		/// based on the current delay, or lack thereof
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::add_transfer_allowance_no_existing_metadata().max(T::WeightInfo::add_transfer_allowance_existing_metadata()))]
		pub fn add_transfer_allowance(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			receiver: T::Location,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			let allowance_details = match Self::get_account_currency_restriction_count_delay(
				&account_id,
				currency_id,
			) {
				Some(AllowanceMetadata {
					current_delay: Some(delay),
					..
				}) => AllowanceDetails {
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
				T::ReserveCurrency::reserve(
					&account_id,
					T::Fees::fee_value(T::AllowanceFeeKey::get()),
				)?;
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

		/// Restricts a transfer allowance for a sending
		/// account/currency/receiver location to:
		/// - either the current block + delay if a delay is set
		/// - or the current block if no delay is set
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::remove_transfer_allowance_missing_allowance().max(T::WeightInfo::remove_transfer_allowance_delay_present()))]
		pub fn remove_transfer_allowance(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			receiver: T::Location,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			let blocked_at = match Self::get_account_currency_restriction_count_delay(
				&account_id,
				currency_id,
			) {
				Some(AllowanceMetadata {
					current_delay: Some(delay),
					..
				}) => <frame_system::Pallet<T>>::block_number().saturating_add(delay),
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

		/// Removes a transfer allowance for a sending account/currency and
		/// receiving location Decrements or removes the sending
		/// account/currency count.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::purge_transfer_allowance_no_remaining_metadata().max(T::WeightInfo::purge_allowance_delay_remaining_metadata()))]
		pub fn purge_transfer_allowance(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			receiver: T::Location,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let current_block = <frame_system::Pallet<T>>::block_number();
			match <AccountCurrencyTransferAllowance<T>>::get((&account_id, &currency_id, &receiver))
			{
				Some(AllowanceDetails { blocked_at, .. }) if blocked_at < current_block => {
					T::ReserveCurrency::unreserve(
						&account_id,
						T::Fees::fee_value(T::AllowanceFeeKey::get()),
					);
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
				Some(_) => Err(DispatchError::from(Error::<T>::AllowanceHasNotExpired)),
				None => Err(DispatchError::from(Error::<T>::NoMatchingAllowance)),
			}
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::add_allowance_delay_existing_metadata().max(T::WeightInfo::add_allowance_delay_no_existing_metadata()))]
		/// Adds an account/currency delay
		/// Calling on an account/currency with an existing delay will fail.
		/// To update a delay the delay has to be set to future modifiable.
		/// then an update delay extrinsic called
		pub fn add_allowance_delay(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			delay: BlockNumberFor<T>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let count_delay = match Self::get_account_currency_restriction_count_delay(
				&account_id,
				currency_id,
			) {
				None => Ok(AllowanceMetadata {
					allowance_count: 0,
					current_delay: Some(delay),
					once_modifiable_after: None,
				}),
				Some(
					metadata @ AllowanceMetadata {
						current_delay: None,
						..
					},
				) => Ok(AllowanceMetadata {
					current_delay: Some(delay),
					..metadata
				}),
				Some(AllowanceMetadata {
					current_delay: Some(_),
					..
				}) => Err(DispatchError::from(Error::<T>::DuplicateDelay)),
			}?;

			<AccountCurrencyTransferCountDelay<T>>::insert(&account_id, currency_id, count_delay);
			Self::deposit_event(Event::TransferAllowanceDelayAdd {
				sender_account_id: account_id,
				currency_id,
				delay,
			});
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::update_allowance_delay())]
		/// Updates an allowance delay, only callable if the delay has been set
		/// to allow future modifications and the delay modifiable_at block has
		/// been passed.
		pub fn update_allowance_delay(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			delay: BlockNumberFor<T>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let current_block = <frame_system::Pallet<T>>::block_number();
			match Self::get_account_currency_restriction_count_delay(&account_id, currency_id) {
				None => Err(DispatchError::from(Error::<T>::NoMatchingDelay)),
				Some(AllowanceMetadata {
					current_delay: None,
					..
				}) => Err(DispatchError::from(Error::<T>::NoMatchingDelay)),
				Some(AllowanceMetadata {
					once_modifiable_after: None,
					..
				}) => Err(DispatchError::from(Error::<T>::DelayUnmodifiable)),
				Some(AllowanceMetadata {
					once_modifiable_after: Some(modifiable_at),
					..
				}) if current_block < modifiable_at => Err(DispatchError::from(Error::<T>::DelayUnmodifiable)),
				Some(metadata) => {
					<AccountCurrencyTransferCountDelay<T>>::insert(
						&account_id,
						currency_id,
						AllowanceMetadata {
							current_delay: Some(delay),
							// we want to ensure that after the delay is modified, it cannot be
							// modified on a whim without another modifiable_at set.
							once_modifiable_after: None,
							..metadata
						},
					);
					Self::deposit_event(Event::TransferAllowanceDelayUpdate {
						sender_account_id: account_id,
						currency_id,
						delay,
					});
					Ok(())
				}
			}
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::toggle_allowance_delay_once_future_modifiable())]
		/// This allows the delay value to be modified after the current delay
		/// has passed since the current block Or sets the delay value to be not
		/// modifiable iff modifiable at has already passed
		pub fn toggle_allowance_delay_once_future_modifiable(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let current_block = <frame_system::Pallet<T>>::block_number();
			let metadata = match Self::get_account_currency_restriction_count_delay(
				&account_id,
				currency_id,
			) {
				None => Err(DispatchError::from(Error::<T>::NoMatchingDelay)),
				Some(AllowanceMetadata {
					current_delay: None,
					..
				}) => Err(DispatchError::from(Error::<T>::NoMatchingDelay)),
				Some(AllowanceMetadata {
					once_modifiable_after: Some(modifiable_at),
					..
				}) if modifiable_at > current_block => Err(DispatchError::from(Error::<T>::DelayUnmodifiable)),
				Some(
					metadata @ AllowanceMetadata {
						once_modifiable_after: Some(_),
						..
					},
				) => Ok(AllowanceMetadata {
					once_modifiable_after: None,
					..metadata
				}),
				Some(
					metadata @ AllowanceMetadata {
						current_delay: Some(current_delay),
						..
					},
				) => Ok(AllowanceMetadata {
					once_modifiable_after: Some(current_block.ensure_add(current_delay)?),
					..metadata
				}),
			}?;
			<AccountCurrencyTransferCountDelay<T>>::insert(&account_id, currency_id, metadata);
			Self::deposit_event(Event::ToggleTransferAllowanceDelayFutureModifiable {
				sender_account_id: account_id,
				currency_id,
				modifiable_once_after: metadata.once_modifiable_after,
			});
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::purge_allowance_delay_remaining_metadata().max(T::WeightInfo::purge_allowance_delay_no_remaining_metadata()))]
		/// Removes an existing sending account/currency delay
		pub fn purge_allowance_delay(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			let current_block = <frame_system::Pallet<T>>::block_number();
			match Self::get_account_currency_restriction_count_delay(&account_id, currency_id) {
				Some(AllowanceMetadata {
					allowance_count: 0,
					once_modifiable_after: Some(modifiable_at),
					..
				}) if modifiable_at < current_block => {
					<AccountCurrencyTransferCountDelay<T>>::remove(&account_id, currency_id);
					Self::deposit_event(Event::TransferAllowanceDelayPurge {
						sender_account_id: account_id,
						currency_id,
					});
					Ok(())
				}
				Some(
					metadata @ AllowanceMetadata {
						once_modifiable_after: Some(modifiable_at),
						..
					},
				) if modifiable_at <= current_block => {
					<AccountCurrencyTransferCountDelay<T>>::insert(
						&account_id,
						currency_id,
						AllowanceMetadata {
							current_delay: None,
							once_modifiable_after: None,
							..metadata
						},
					);
					Self::deposit_event(Event::TransferAllowanceDelayPurge {
						sender_account_id: account_id,
						currency_id,
					});
					Ok(())
				}
				None => Err(DispatchError::from(Error::<T>::NoMatchingDelay)),
				_ => Err(DispatchError::from(Error::<T>::DelayUnmodifiable)),
			}
		}
	}

	impl<T: Config> Pallet<T> {
		/// Increments number of allowances present for a sending
		/// account/currency set. If no allowances set, an entry with 1 added,
		/// if entry already present, it is then incremented.
		pub fn increment_or_create_allowance_count(
			account_id: &T::AccountId,
			currency_id: &T::CurrencyId,
		) -> DispatchResult {
			// not using try_mutate here as we're not sure if key exits, and we're already
			// doing a some value check on result of exists query check
			match Self::get_account_currency_restriction_count_delay(account_id, currency_id) {
				Some(
					metadata @ AllowanceMetadata {
						allowance_count, ..
					},
				) => {
					let new_allowance_count = allowance_count.ensure_add(1)?;
					<AccountCurrencyTransferCountDelay<T>>::insert(
						account_id,
						currency_id,
						AllowanceMetadata {
							allowance_count: new_allowance_count,
							..metadata
						},
					);
					Ok(())
				}
				_ => {
					<AccountCurrencyTransferCountDelay<T>>::insert(
						account_id,
						currency_id,
						AllowanceMetadata::default(),
					);
					Ok(())
				}
			}
		}

		/// Decrements the number of allowances tracked for a sending
		/// account/currency set. If the allowance count is currently 1, then it
		/// removes the entry If greater than 1, then decremented.
		/// If no entry present, NoAllowancesSet error returned.
		pub fn decrement_or_remove_allowance_count(
			account_id: &T::AccountId,
			currency_id: &T::CurrencyId,
		) -> DispatchResult {
			// not using try_mutate here as we're not sure if key exits, and we're already
			// doing a some value check on result of exists query check
			match Self::get_account_currency_restriction_count_delay(account_id, currency_id) {
				Some(AllowanceMetadata {
					allowance_count,
					current_delay: None,
					once_modifiable_after: None,
				}) if allowance_count <= 1 => {
					<AccountCurrencyTransferCountDelay<T>>::remove(account_id, currency_id);
					Ok(())
				}
				Some(
					metadata @ AllowanceMetadata {
						allowance_count, ..
					},
				) if allowance_count <= 1 => {
					<AccountCurrencyTransferCountDelay<T>>::insert(
						account_id,
						currency_id,
						AllowanceMetadata {
							allowance_count: 0,
							..metadata
						},
					);
					Ok(())
				}
				Some(
					metadata @ AllowanceMetadata {
						allowance_count, ..
					},
				) => {
					// check in this case should not ever be needed
					let new_allowance_count = allowance_count.ensure_sub(1)?;
					<AccountCurrencyTransferCountDelay<T>>::insert(
						account_id,
						currency_id,
						AllowanceMetadata {
							allowance_count: new_allowance_count,
							..metadata
						},
					);
					Ok(())
				}
				_ => Err(DispatchError::from(Error::<T>::NoAllowancesSet)),
			}
		}
	}

	impl<T: Config> TransferAllowance<T::AccountId> for Pallet<T> {
		type CurrencyId = T::CurrencyId;
		type Location = T::Location;

		/// This checks to see if a transfer from an account and currency should
		/// be allowed to a given location. If there are no allowances defined
		/// for the sending account and currency, then the transfer is allowed.
		/// If there is an allowance for the sending account and currency,
		/// but the destination does not have an allowance added then the
		/// transfer is not allowed. If there is an allowance for the sending
		/// account and currency, and there's an allowance present:
		/// then we check whether the current block is between the `allowed_at`
		/// and `blocked_at` blocks in the allowance.
		fn allowance(
			send: T::AccountId,
			receive: Self::Location,
			currency: T::CurrencyId,
		) -> DispatchResult {
			match Self::get_account_currency_restriction_count_delay(&send, currency) {
				Some(AllowanceMetadata {
					allowance_count: count,
					..
				}) if count > 0 => {
					let current_block = <frame_system::Pallet<T>>::block_number();
					match <AccountCurrencyTransferAllowance<T>>::get((&send, &currency, receive)) {
						Some(AllowanceDetails {
							allowed_at,
							blocked_at,
						}) if current_block >= allowed_at && current_block < blocked_at => Ok(()),
						_ => Err(DispatchError::from(Error::<T>::NoAllowanceForDestination)),
					}
				}
				// In this case no allowances are set for the sending account & currency,
				// therefore no restrictions should be in place.
				_ => Ok(()),
			}
		}
	}
}
