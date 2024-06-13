// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;
pub use pallet::*;
use scale_info::TypeInfo;
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Anchor<AccountId, DocumentId, DocumentVersion, Hash, Balance> {
	account_id: AccountId,
	document_id: DocumentId,
	document_version: DocumentVersion,
	hash: Hash,
	deposit: Balance,
}

pub type AnchorOf<T> = Anchor<
	<T as frame_system::Config>::AccountId,
	<T as Config>::DocumentId,
	<T as Config>::DocumentVersion,
	<T as frame_system::Config>::Hash,
	<T as Config>::Balance,
>;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::traits::ReservableCurrency;
	use frame_system::pallet_prelude::*;
	use sp_runtime::{traits::AtLeast32BitUnsigned, FixedPointOperand};

	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u64>
			+ From<u128>
			+ TypeInfo
			+ TryInto<u64>;

		type Currency: ReservableCurrency<Self::AccountId, Balance = Self::Balance>;

		/// Default deposit that will be taken when adding an anchor.
		type DefaultAnchorDeposit: Get<Self::Balance>;

		/// The type used to identify a document.
		type DocumentId: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u64>
			+ From<u128>
			+ TypeInfo
			+ TryInto<u64>;

		/// The type used to version a document.
		type DocumentVersion: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u64>
			+ TypeInfo
			+ TryInto<u64>;

		/// Origin used when setting a deposit.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Storage for document anchors.
	#[pallet::storage]
	#[pallet::getter(fn get_anchor)]
	pub type Anchors<T: Config> = StorageDoubleMap<
		_,
		Blake2_256,
		(T::DocumentId, T::DocumentVersion),
		Blake2_256,
		T::AccountId,
		AnchorOf<T>,
	>;

	/// Storage for document anchors specific to an account.
	#[pallet::storage]
	#[pallet::getter(fn get_personal_anchor)]
	pub type PersonalAnchors<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Blake2_256, T::AccountId>,
			NMapKey<Blake2_256, T::DocumentId>,
			NMapKey<Blake2_256, T::DocumentVersion>,
		),
		AnchorOf<T>,
		OptionQuery,
	>;

	/// Stores the current deposit that will be taken when storing an anchor.
	#[pallet::storage]
	#[pallet::getter(fn get_anchor_deposit)]
	pub type AnchorDeposit<T: Config> =
		StorageValue<_, T::Balance, ValueQuery, T::DefaultAnchorDeposit>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An anchor was added.
		AnchorAdded {
			account_id: T::AccountId,
			document_id: T::DocumentId,
			document_version: T::DocumentVersion,
			hash: T::Hash,
			deposit: T::Balance,
		},
		/// An anchor was removed.
		AnchorRemoved {
			account_id: T::AccountId,
			document_id: T::DocumentId,
			document_version: T::DocumentVersion,
			hash: T::Hash,
			deposit: T::Balance,
		},
		/// A deposit was set.
		DepositSet { new_deposit: T::Balance },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The anchor already exists.
		AnchorAlreadyExists,

		/// The personal anchor already exists.
		PersonalAnchorAlreadyExists,

		/// The anchor was not found in storage.
		AnchorNotFound,

		/// The personal anchor was not found in storage.
		PersonalAnchorNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets an anchor for a document ID and version.
		#[pallet::weight(T::WeightInfo::set_anchor())]
		#[pallet::call_index(0)]
		pub fn set_anchor(
			origin: OriginFor<T>,
			document_id: T::DocumentId,
			document_version: T::DocumentVersion,
			hash: T::Hash,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			// Only one anchor should be stored for a particular document ID and version.
			ensure!(
				Anchors::<T>::iter_prefix_values((document_id, document_version)).count() == 0,
				Error::<T>::AnchorAlreadyExists
			);

			ensure!(
				PersonalAnchors::<T>::get((account_id.clone(), document_id, document_version))
					.is_none(),
				Error::<T>::PersonalAnchorAlreadyExists
			);

			let deposit = AnchorDeposit::<T>::get();

			T::Currency::reserve(&account_id, deposit)?;

			let anchor = AnchorOf::<T> {
				account_id: account_id.clone(),
				document_id,
				document_version,
				hash,
				deposit,
			};

			Anchors::<T>::insert(
				(document_id, document_version),
				account_id.clone(),
				anchor.clone(),
			);

			PersonalAnchors::<T>::insert(
				(account_id.clone(), document_id, document_version),
				anchor,
			);

			Self::deposit_event(Event::AnchorAdded {
				account_id,
				document_id,
				document_version,
				hash,
				deposit,
			});

			Ok(())
		}

		/// Removes an anchor for a document ID and version.
		#[pallet::weight(T::WeightInfo::remove_anchor())]
		#[pallet::call_index(1)]
		pub fn remove_anchor(
			origin: OriginFor<T>,
			document_id: T::DocumentId,
			document_version: T::DocumentVersion,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			ensure!(
				PersonalAnchors::<T>::get((account_id.clone(), document_id, document_version))
					.is_some(),
				Error::<T>::PersonalAnchorNotFound
			);

			let anchor = Anchors::<T>::get((document_id, document_version), account_id.clone())
				.ok_or(Error::<T>::AnchorNotFound)?;

			T::Currency::unreserve(&account_id, anchor.deposit);

			Anchors::<T>::remove((document_id, document_version), account_id.clone());
			PersonalAnchors::<T>::remove((account_id.clone(), document_id, document_version));

			Self::deposit_event(Event::AnchorRemoved {
				account_id,
				document_id,
				document_version,
				hash: anchor.hash,
				deposit: anchor.deposit,
			});

			Ok(())
		}

		/// Set a new anchor deposit.
		#[pallet::weight(T::WeightInfo::set_deposit())]
		#[pallet::call_index(2)]
		pub fn set_deposit(origin: OriginFor<T>, new_deposit: T::Balance) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			<AnchorDeposit<T>>::set(new_deposit);

			Self::deposit_event(Event::DepositSet { new_deposit });

			Ok(())
		}
	}
}
