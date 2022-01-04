#![cfg_attr(not(feature = "std"), no_std)]

extern crate common_traits;
extern crate frame_benchmarking;
extern crate frame_support;
extern crate sp_runtime;

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::*;

pub trait Permissions<AccountId> {
	type Role;
	type Storage: Properties<Property = Self::Role, Element = Self::Storage>;
	type Error;

	fn roles(who: AccountId) -> Option<Self::Storage>;

	fn clearance(who: AccountId, role: Self::Role) -> bool;

	fn add_permission(who: AccountId, role: Self::Role) -> Result<(), Self::Error>;

	fn rm_permission(who: AccountId, role: Self::Role) -> Result<(), Self::Error>;
}

pub trait Properties {
	type Property;
	type Element;

	fn exists(element: &Self::Element, property: Self::Property) -> bool;

	fn rm(element: &mut Self::Element, property: Self::Property);

	fn add(element: &mut Self::Element, property: Self::Property);
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Role: Member + Parameter;

		type Storage: Member
			+ Parameter
			+ Properties<Property = Self::Role, Element = Self::Storage>
			+ Default;

		type AdminOrigin: EnsureOrigin<Self::Origin>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn permission)]
	pub type Permission<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::Storage>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		RoleAdded(T::AccountId, T::Role),
		RoleRemoved(T::AccountId, T::Role),
		ClearancePurged(T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		RoleAlreadyGiven,
		RoleNotGiven,
		NoRoles,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn add_permission(
			origin: OriginFor<T>,
			to: T::AccountId,
			role: T::Role,
		) -> DispatchResult {
			Self::ensure_admin(origin)?;

			Pallet::<T>::do_add_permission(to.clone(), role.clone())
				.map(|_| Self::deposit_event(Event::<T>::RoleAdded(to, role)))?;

			Ok(())
		}

		#[pallet::weight(100)]
		pub fn rm_permission(
			origin: OriginFor<T>,
			from: T::AccountId,
			role: T::Role,
		) -> DispatchResult {
			Self::ensure_admin(origin)?;

			Pallet::<T>::do_rm_permission(from.clone(), role.clone())
				.map(|_| Self::deposit_event(Event::<T>::RoleRemoved(from, role)))?;

			Ok(())
		}

		#[pallet::weight(100)]
		pub fn rm_clearance(origin: OriginFor<T>, from: T::AccountId) -> DispatchResult {
			Self::ensure_admin(origin)?;

			Permission::<T>::remove(from.clone());

			Self::deposit_event(Event::<T>::ClearancePurged(from));

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn ensure_admin(origin: OriginFor<T>) -> DispatchResult {
		T::AdminOrigin::ensure_origin(origin)
			.map_or(Err(DispatchError::BadOrigin), |_| Ok(().into()))
	}

	fn do_add_permission(who: T::AccountId, role: T::Role) -> Result<(), Error<T>> {
		Permission::<T>::try_get(who.clone()).map_or(
			{
				let mut def = T::Storage::default();
				<<T as Config>::Storage as Properties>::add(&mut def, role.clone());

				Ok(Permission::<T>::insert(who.clone(), def))
			},
			|mut roles| {
				if !<<T as Config>::Storage as Properties>::exists(&roles, role.clone()) {
					<<T as Config>::Storage as Properties>::add(&mut roles, role);

					Ok(Permission::<T>::insert(who.clone(), roles))
				} else {
					Err(Error::<T>::RoleAlreadyGiven)
				}
			},
		)
	}

	fn do_rm_permission(who: T::AccountId, role: T::Role) -> Result<(), Error<T>> {
		Permission::<T>::try_get(who.clone()).map_or(Err(Error::<T>::NoRoles), |mut roles| {
			if <<T as Config>::Storage as Properties>::exists(&roles, role.clone()) {
				<<T as Config>::Storage as Properties>::rm(&mut roles, role);

				Ok(Permission::<T>::insert(who.clone(), roles))
			} else {
				Err(Error::<T>::RoleNotGiven)
			}
		})
	}
}

impl<T: Config> Permissions<T::AccountId> for Pallet<T> {
	type Role = T::Role;
	type Storage = T::Storage;
	type Error = Error<T>;

	fn roles(who: T::AccountId) -> Option<T::Storage> {
		Permission::<T>::get(who)
	}

	fn clearance(who: T::AccountId, role: T::Role) -> bool {
		Permission::<T>::get(who).map_or(false, |roles| {
			<<T as Config>::Storage as Properties>::exists(&roles, role)
		})
	}

	fn add_permission(who: T::AccountId, role: T::Role) -> Result<(), Error<T>> {
		Pallet::<T>::do_add_permission(who, role)
	}

	fn rm_permission(who: T::AccountId, role: T::Role) -> Result<(), Error<T>> {
		Pallet::<T>::do_rm_permission(who, role)
	}
}
