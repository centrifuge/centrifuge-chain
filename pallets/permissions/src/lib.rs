// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

///! A crate that defines a simple permissions logic for our infrastructure.
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

/// Who informs about the caller's role
enum Who {
	Admin,
	Editor,
}

use common_traits::{Permissions, Properties};
use frame_support::traits::Contains;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Location: Member + Parameter;

		type Role: Member + Parameter;

		type Storage: Member + Parameter + Properties<Property = Self::Role> + Default;

		type Editors: Contains<(
			Self::AccountId,
			Option<Self::Role>,
			Self::Location,
			Self::Role,
		)>;

		type AdminOrigin: EnsureOrigin<Self::Origin>;

		type MaxRolesPerLocation: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn permission)]
	pub type Permission<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::Location,
		T::Storage,
	>;

	#[pallet::storage]
	#[pallet::getter(fn permission_count)]
	pub type PermissionCount<T: Config> = StorageMap<_, Blake2_128Concat, T::Location, u32>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Added(T::AccountId, T::Location, T::Role),
		Removed(T::AccountId, T::Location, T::Role),
		Purged(T::AccountId, T::Location),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		RoleAlreadyGiven,
		RoleNotGiven,
		NoRoles,
		NoEditor,
		WrongParameters,
		TooManyRoles,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::add_as_admin().max(T::WeightInfo::add_as_editor()))]
		pub fn add(
			origin: OriginFor<T>,
			with_role: T::Role,
			to: T::AccountId,
			location: T::Location,
			role: T::Role,
		) -> DispatchResultWithPostInfo {
			let who =
				Self::ensure_admin_or_editor(origin, with_role, location.clone(), role.clone())?;

			Pallet::<T>::do_add(location.clone(), to.clone(), role.clone())
				.map(|_| Self::deposit_event(Event::<T>::Added(to, location, role)))?;

			match who {
				Who::Editor => Ok(Some(T::WeightInfo::add_as_editor()).into()),
				Who::Admin => Ok(Some(T::WeightInfo::add_as_admin()).into()),
			}
		}

		#[pallet::weight(T::WeightInfo::remove_as_editor().max(T::WeightInfo::remove_as_admin()))]
		pub fn remove(
			origin: OriginFor<T>,
			with_role: T::Role,
			from: T::AccountId,
			location: T::Location,
			role: T::Role,
		) -> DispatchResultWithPostInfo {
			let who =
				Self::ensure_admin_or_editor(origin, with_role, location.clone(), role.clone())?;

			Pallet::<T>::do_remove(location.clone(), from.clone(), role.clone())
				.map(|_| Self::deposit_event(Event::<T>::Removed(from, location, role)))?;

			match who {
				Who::Editor => Ok(Some(T::WeightInfo::remove_as_editor()).into()),
				Who::Admin => Ok(Some(T::WeightInfo::remove_as_admin()).into()),
			}
		}

		#[pallet::weight(T::WeightInfo::purge())]
		pub fn purge(origin: OriginFor<T>, location: T::Location) -> DispatchResult {
			let from = ensure_signed(origin)?;

			ensure!(
				Permission::<T>::contains_key(from.clone(), location.clone()),
				Error::<T>::NoRoles
			);

			Permission::<T>::remove(from.clone(), location.clone());

			Self::deposit_event(Event::<T>::Purged(from, location));

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::admin_purge())]
		pub fn admin_purge(
			origin: OriginFor<T>,
			from: T::AccountId,
			location: T::Location,
		) -> DispatchResult {
			Self::ensure_admin(origin)?;

			ensure!(
				Permission::<T>::contains_key(from.clone(), location.clone()),
				Error::<T>::NoRoles
			);

			Permission::<T>::remove(from.clone(), location.clone());

			Self::deposit_event(Event::<T>::Purged(from, location));

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn ensure_admin_or_editor(
		origin: OriginFor<T>,
		with_role: T::Role,
		location: T::Location,
		role: T::Role,
	) -> Result<Who, DispatchError> {
		// check if origin is admin
		match Self::ensure_admin(origin.clone()) {
			Ok(()) => Ok(Who::Admin),
			_ => {
				// check if origin is editor
				let editor = ensure_signed(origin)?;
				let is_editor = Permission::<T>::get(editor.clone(), location.clone())
					.and_then(|roles| {
						Some(
							roles.exists(with_role.clone())
								&& T::Editors::contains(&(editor, Some(with_role), location, role)),
						)
					})
					.unwrap_or(false);
				ensure!(is_editor, Error::<T>::NoEditor);
				Ok(Who::Editor)
			}
		}
	}

	fn ensure_admin(origin: OriginFor<T>) -> DispatchResult {
		T::AdminOrigin::ensure_origin(origin).map_or(Err(Error::<T>::NoEditor.into()), |_| Ok(()))
	}

	fn do_add(
		location: T::Location,
		who: T::AccountId,
		role: T::Role,
	) -> Result<(), DispatchError> {
		PermissionCount::<T>::try_mutate(location.clone(), |perm_count| {
			let num_permissions = perm_count.map_or(1, |count| count + 1);
			if num_permissions > T::MaxRolesPerLocation::get() {
				return Err(Error::<T>::TooManyRoles.into());
			}
			*perm_count = Some(num_permissions);

			Permission::<T>::try_mutate(who.clone(), location.clone(), |maybe_roles| {
				let mut roles = maybe_roles.take().unwrap_or_default();
				if roles.exists(role.clone()) {
					Err(Error::<T>::RoleAlreadyGiven.into())
				} else {
					roles
						.add(role.clone())
						.map_err(|_| Error::<T>::WrongParameters)?;
					*maybe_roles = Some(roles);
					Ok(())
				}
			})
		})
	}

	fn do_remove(
		location: T::Location,
		who: T::AccountId,
		role: T::Role,
	) -> Result<(), DispatchError> {
		PermissionCount::<T>::try_mutate(location.clone(), |perm_count| {
			let num_permissions = perm_count.map_or(0, |count| count - 1);
			if num_permissions == 0 {
				*perm_count = None;
			} else {
				*perm_count = Some(num_permissions);
			}

			Permission::<T>::try_mutate(who.clone(), location.clone(), |maybe_roles| {
				let mut roles = maybe_roles.take().ok_or(Error::<T>::NoRoles)?;
				if roles.exists(role.clone()) {
					roles.rm(role).map_err(|_| Error::<T>::WrongParameters)?;
					if roles.empty() {
						*maybe_roles = None
					} else {
						*maybe_roles = Some(roles)
					}
					Ok(())
				} else {
					Err(Error::<T>::RoleNotGiven.into())
				}
			})
		})
	}
}

impl<T: Config> Permissions<T::AccountId> for Pallet<T> {
	type Location = T::Location;
	type Role = T::Role;
	type Error = DispatchError;
	type Ok = ();

	fn has(location: T::Location, who: T::AccountId, role: T::Role) -> bool {
		Permission::<T>::get(who, location).map_or(false, |roles| roles.exists(role))
	}

	fn add(location: T::Location, who: T::AccountId, role: T::Role) -> Result<(), DispatchError> {
		Pallet::<T>::do_add(location, who, role)
	}

	fn remove(
		location: T::Location,
		who: T::AccountId,
		role: T::Role,
	) -> Result<(), DispatchError> {
		Pallet::<T>::do_remove(location, who, role)
	}
}
