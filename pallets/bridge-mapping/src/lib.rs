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

//! # Bridge Access Control List Pallet
//!
//! This pallet provides functionality for setting and tracking allowed paths for assets to be
//! transferred across chains, or more abstractly, resources. A resource is a name that represents
//! an abstract concept, like an asset that exists across different blockchains.
//!
//! For example, Eth may be an abstract resource, with instances of it being the native token on
//! Ethereum, and also a derivative token on some bridged Substrate blockchain.
//!
//! Resources are set and removed by an Admin account or by root.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::EnsureOrigin;
use frame_system::ensure_root;
use sp_runtime::traits::BadOrigin;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_nft::Config {
		/// A local mapping of a resource id.
		///
		/// This associated type represents anything that a resource id might map to.
		/// On Ethereum, for instance, this may be a contract address for transferring
		/// assets.
		type Address: Parameter
			+ Member
			+ Default
			+ From<<Self as pallet_nft::Config>::RegistryId>
			+ Into<<Self as pallet_nft::Config>::RegistryId>;

		/// Admin is able to set/remove resource mappings.
		type AdminOrigin: EnsureOrigin<Self::Origin>;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	// The genesis config type.
	#[pallet::genesis_config]
	pub struct GenesisConfig {}

	// The default value for the genesis config type.
	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self {}
		}
	}

	// The build of genesis for the pallet.
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {}
	}

	/// Indicates that assets of a resource can be transferred to another resource.
	/// Maps an abstract resource id to a chain-specific address
	#[pallet::storage]
	#[pallet::getter(fn addr_of)]
	pub(super) type ResourceToAddress<T: Config> =
		StorageMap<_, Blake2_128Concat, T::ResourceId, T::Address>;

	/// Maps a chain-specific address to a resource id. A mapping in [ResourceToAddress] will
	/// always correspond to a mapping here. Resources and addresses are 1 to 1.
	#[pallet::storage]
	#[pallet::getter(fn name_of)]
	pub(super) type AddressToResource<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Address, T::ResourceId>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set a resource mapping in the [Names]. Existing keys will be overwritten.
		/// The caller must be the owner of the `rid` ResourceId.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set())]
		pub fn set(
			origin: OriginFor<T>,
			rid: T::ResourceId,
			local_addr: T::Address,
		) -> DispatchResult {
			Self::ensure_admin_or_root(origin)?;

			// Call internal
			Self::set_resource(rid, local_addr);
			Ok(())
		}

		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove())]
		pub fn remove(origin: OriginFor<T>, rid: T::ResourceId) -> DispatchResult {
			Self::ensure_admin_or_root(origin)?;

			// Call internal
			Self::remove_resource(&rid);
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Ensure that the given origin is either the pallet [AdminOrigin] or frame_system root.
	fn ensure_admin_or_root(origin: T::Origin) -> Result<(), BadOrigin> {
		<T as pallet::Config>::AdminOrigin::try_origin(origin)
			.map(|_| ())
			.or_else(ensure_root)
	}

	/// Add a new resource mapping in [Names]. Existing entries will be overwritten.
	pub fn set_resource(rid: T::ResourceId, local_addr: T::Address) {
		// Add the mapping both ways
		<ResourceToAddress<T>>::insert(rid.clone(), local_addr.clone());
		<AddressToResource<T>>::insert(local_addr, rid);
	}

	/// Remove a resource mapping in [Names].
	pub fn remove_resource(rid: &T::ResourceId) {
		// If it doesn't exist for some unexpected reason, still allow removal by setting default
		let address = <ResourceToAddress<T>>::get(rid).unwrap_or_default();
		// Remove the resource mapping both ways
		<ResourceToAddress<T>>::remove(rid);
		<AddressToResource<T>>::remove(address);
	}
}
