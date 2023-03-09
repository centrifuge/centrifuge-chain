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
	/// Admins can add/remove permissions,
	/// and purge other users permissions.
	Admin,
	/// Editors can add/remove permissions
	Editor,
}

use cfg_traits::{Permissions, Properties};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::Contains};
use frame_system::pallet_prelude::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Scope: Member + Parameter + MaxEncodedLen;

		type Role: Member + Parameter;

		type Storage: Member
			+ Parameter
			+ Properties<Property = Self::Role>
			+ Default
			+ MaxEncodedLen;

		type Editors: Contains<(Self::AccountId, Option<Self::Role>, Self::Scope, Self::Role)>;

		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		#[pallet::constant]
		type MaxRolesPerScope: Get<u32>;

		/// The maximum number of tranches.
		#[pallet::constant]
		type MaxTranches: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn permission)]
	pub type Permission<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, T::Scope, T::Storage>;

	#[pallet::storage]
	#[pallet::getter(fn permission_count)]
	pub type PermissionCount<T: Config> = StorageMap<_, Blake2_128Concat, T::Scope, u32>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Added {
			to: T::AccountId,
			scope: T::Scope,
			role: T::Role,
		},
		Removed {
			from: T::AccountId,
			scope: T::Scope,
			role: T::Role,
		},
		Purged {
			from: T::AccountId,
			scope: T::Scope,
		},
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
		#[pallet::call_index(0)]
		pub fn add(
			origin: OriginFor<T>,
			with_role: T::Role,
			to: T::AccountId,
			scope: T::Scope,
			role: T::Role,
		) -> DispatchResultWithPostInfo {
			let who = Self::ensure_admin_or_editor(origin, with_role, scope.clone(), role.clone())?;

			Pallet::<T>::do_add(scope, to, role)?;

			match who {
				Who::Editor => Ok(Some(T::WeightInfo::add_as_editor()).into()),
				Who::Admin => Ok(Some(T::WeightInfo::add_as_admin()).into()),
			}
		}

		#[pallet::weight(T::WeightInfo::remove_as_editor().max(T::WeightInfo::remove_as_admin()))]
		#[pallet::call_index(1)]
		pub fn remove(
			origin: OriginFor<T>,
			with_role: T::Role,
			from: T::AccountId,
			scope: T::Scope,
			role: T::Role,
		) -> DispatchResultWithPostInfo {
			let who = Self::ensure_admin_or_editor(origin, with_role, scope.clone(), role.clone())?;

			Pallet::<T>::do_remove(scope, from, role)?;

			match who {
				Who::Editor => Ok(Some(T::WeightInfo::remove_as_editor()).into()),
				Who::Admin => Ok(Some(T::WeightInfo::remove_as_admin()).into()),
			}
		}

		#[pallet::weight(T::WeightInfo::purge())]
		#[pallet::call_index(2)]
		pub fn purge(origin: OriginFor<T>, scope: T::Scope) -> DispatchResult {
			let from = ensure_signed(origin)?;

			ensure!(
				Permission::<T>::contains_key(from.clone(), scope.clone()),
				Error::<T>::NoRoles
			);

			Permission::<T>::remove(from.clone(), scope.clone());

			Self::deposit_event(Event::<T>::Purged { from, scope });

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::admin_purge())]
		#[pallet::call_index(3)]
		pub fn admin_purge(
			origin: OriginFor<T>,
			from: T::AccountId,
			scope: T::Scope,
		) -> DispatchResult {
			Self::ensure_admin(origin)?;

			ensure!(
				Permission::<T>::contains_key(from.clone(), scope.clone()),
				Error::<T>::NoRoles
			);

			Permission::<T>::remove(from.clone(), scope.clone());

			Self::deposit_event(Event::<T>::Purged { from, scope });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn ensure_admin_or_editor(
		origin: OriginFor<T>,
		with_role: T::Role,
		scope: T::Scope,
		role: T::Role,
	) -> Result<Who, DispatchError> {
		// check if origin is admin
		match Self::ensure_admin(origin.clone()) {
			Ok(()) => Ok(Who::Admin),
			_ => {
				// check if origin is editor
				let editor = ensure_signed(origin)?;
				let is_editor = Permission::<T>::get(editor.clone(), scope.clone())
					.map(|roles| {
						roles.exists(with_role.clone())
							&& T::Editors::contains(&(editor, Some(with_role), scope, role))
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

	fn do_add(scope: T::Scope, to: T::AccountId, role: T::Role) -> DispatchResult {
		PermissionCount::<T>::try_mutate(scope.clone(), |perm_count| {
			let num_permissions = perm_count.map_or(1, |count| count + 1);
			if num_permissions > T::MaxRolesPerScope::get() {
				return Err(Error::<T>::TooManyRoles.into());
			}
			*perm_count = Some(num_permissions);

			Permission::<T>::try_mutate(
				to.clone(),
				scope.clone(),
				|maybe_roles| -> DispatchResult {
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
				},
			)
		})?;

		Self::deposit_event(Event::<T>::Added { to, scope, role });
		Ok(())
	}

	fn do_remove(scope: T::Scope, from: T::AccountId, role: T::Role) -> DispatchResult {
		PermissionCount::<T>::try_mutate(scope.clone(), |perm_count| {
			let num_permissions = perm_count.map_or(0, |count| count - 1);
			if num_permissions == 0 {
				*perm_count = None;
			} else {
				*perm_count = Some(num_permissions);
			}

			Permission::<T>::try_mutate(
				from.clone(),
				scope.clone(),
				|maybe_roles| -> DispatchResult {
					let mut roles = maybe_roles.take().ok_or(Error::<T>::NoRoles)?;
					if roles.exists(role.clone()) {
						roles
							.rm(role.clone())
							.map_err(|_| Error::<T>::WrongParameters)?;
						if roles.empty() {
							*maybe_roles = None
						} else {
							*maybe_roles = Some(roles)
						}
						Ok(())
					} else {
						Err(Error::<T>::RoleNotGiven.into())
					}
				},
			)
		})?;

		Self::deposit_event(Event::<T>::Removed { from, scope, role });
		Ok(())
	}
}

impl<T: Config> Permissions<T::AccountId> for Pallet<T> {
	type Error = DispatchError;
	type Ok = ();
	type Role = T::Role;
	type Scope = T::Scope;

	fn has(scope: T::Scope, who: T::AccountId, role: T::Role) -> bool {
		Permission::<T>::get(who, scope).map_or(false, |roles| roles.exists(role))
	}

	fn add(scope: T::Scope, who: T::AccountId, role: T::Role) -> Result<(), DispatchError> {
		Pallet::<T>::do_add(scope, who, role)
	}

	fn remove(scope: T::Scope, who: T::AccountId, role: T::Role) -> Result<(), DispatchError> {
		Pallet::<T>::do_remove(scope, who, role)
	}
}
