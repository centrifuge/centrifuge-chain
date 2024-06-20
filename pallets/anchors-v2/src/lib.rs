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
use frame_support::traits::ReservableCurrency;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::{AtLeast32BitUnsigned, EnsureAdd};

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

pub use pallet::*;
pub use weights::*;

/// Document ID type.
pub type DocumentId = u128;

/// Document version type.
pub type DocumentVersion = u64;

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct Anchor<T: Config> {
	anchor_id: T::AnchorIdNonce,
	account_id: T::AccountId,
	document_id: DocumentId,
	document_version: DocumentVersion,
	hash: T::Hash,
	deposit: T::Balance,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use sp_runtime::traits::{EnsureAddAssign, One};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Balance: frame_support::traits::tokens::Balance;

		type Currency: ReservableCurrency<Self::AccountId, Balance = Self::Balance>;

		/// Default deposit that will be taken when adding an anchor.
		type DefaultAnchorDeposit: Get<Self::Balance>;

		/// Origin used when setting a deposit.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Type used for AnchorId. AnchorIdNonce ensures each
		/// Anchor is unique. AnchorIdNonce is incremented with each new anchor.
		type AnchorIdNonce: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ EnsureAdd
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// Weight information.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Stores AnchorIdNonce and ensure that each anchor has an unique ID.
	#[pallet::storage]
	pub type AnchorIdNonceStore<T: Config> = StorageValue<_, T::AnchorIdNonce, ValueQuery>;

	/// Storage for anchors.
	#[pallet::storage]
	#[pallet::getter(fn get_anchor)]
	pub type Anchors<T: Config> = StorageMap<_, Blake2_256, T::AnchorIdNonce, Anchor<T>>;

	/// Storage for document anchors.
	#[pallet::storage]
	#[pallet::getter(fn get_document_anchor)]
	pub type DocumentAnchors<T: Config> =
		StorageMap<_, Blake2_256, (DocumentId, DocumentVersion), T::AnchorIdNonce>;

	/// Storage for document anchors specific to an account.
	#[pallet::storage]
	#[pallet::getter(fn get_personal_anchor)]
	pub type PersonalAnchors<T: Config> = StorageDoubleMap<
		_,
		Blake2_256,
		T::AccountId,
		Blake2_256,
		(DocumentId, DocumentVersion),
		T::AnchorIdNonce,
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
			anchor_id: T::AnchorIdNonce,
			account_id: T::AccountId,
			document_id: u128,
			document_version: u64,
			hash: T::Hash,
			deposit: T::Balance,
		},
		/// An anchor was removed.
		AnchorRemoved {
			anchor_id: T::AnchorIdNonce,
			account_id: T::AccountId,
			document_id: u128,
			document_version: u64,
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

		/// The document anchor already exists.
		DocumentAnchorAlreadyExists,

		/// The personal anchor already exists.
		PersonalAnchorAlreadyExists,

		/// The anchor was not found in storage.
		AnchorNotFound,

		/// The document anchor was not found in storage.
		DocumentAnchorNotFound,

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
			document_id: u128,
			document_version: u64,
			hash: T::Hash,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			let anchor_id = AnchorIdNonceStore::<T>::try_mutate(|n| {
				n.ensure_add_assign(One::one())?;
				Ok::<_, DispatchError>(*n)
			})?;

			ensure!(
				Anchors::<T>::get(anchor_id).is_none(),
				Error::<T>::AnchorAlreadyExists
			);

			// Only one anchor should be stored for a particular document ID and version.
			ensure!(
				DocumentAnchors::<T>::get((document_id, document_version)).is_none(),
				Error::<T>::DocumentAnchorAlreadyExists
			);
			ensure!(
				PersonalAnchors::<T>::get(account_id.clone(), (document_id, document_version))
					.is_none(),
				Error::<T>::PersonalAnchorAlreadyExists
			);

			let deposit = AnchorDeposit::<T>::get();

			T::Currency::reserve(&account_id, deposit)?;

			let anchor = Anchor::<T> {
				anchor_id,
				account_id: account_id.clone(),
				document_id,
				document_version,
				hash,
				deposit,
			};

			Anchors::<T>::insert(anchor_id, anchor);

			DocumentAnchors::<T>::insert((document_id, document_version), anchor_id);

			PersonalAnchors::<T>::insert(
				account_id.clone(),
				(document_id, document_version),
				anchor_id,
			);

			Self::deposit_event(Event::AnchorAdded {
				anchor_id,
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
			document_id: u128,
			document_version: u64,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			ensure!(
				PersonalAnchors::<T>::get(account_id.clone(), (document_id, document_version))
					.is_some(),
				Error::<T>::PersonalAnchorNotFound
			);

			let anchor_id = DocumentAnchors::<T>::get((document_id, document_version))
				.ok_or(Error::<T>::DocumentAnchorNotFound)?;

			let anchor = Anchors::<T>::get(anchor_id).ok_or(Error::<T>::AnchorNotFound)?;

			T::Currency::unreserve(&account_id, anchor.deposit);

			Anchors::<T>::remove(anchor_id);
			DocumentAnchors::<T>::remove((document_id, document_version));
			PersonalAnchors::<T>::remove(account_id.clone(), (document_id, document_version));

			Self::deposit_event(Event::AnchorRemoved {
				anchor_id,
				account_id,
				document_id,
				document_version,
				hash: anchor.hash,
				deposit: anchor.deposit,
			});

			Ok(())
		}

		/// Set a new anchor deposit.
		#[pallet::weight(T::WeightInfo::set_deposit_value())]
		#[pallet::call_index(2)]
		pub fn set_deposit_value(origin: OriginFor<T>, new_deposit: T::Balance) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			<AnchorDeposit<T>>::set(new_deposit);

			Self::deposit_event(Event::DepositSet { new_deposit });

			Ok(())
		}
	}
}
