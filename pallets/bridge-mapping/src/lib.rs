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

use codec::FullCodec;
use sp_runtime::traits::{Member, BadOrigin};
use frame_system::ensure_root;
use frame_support::{
    decl_module, decl_storage,
    dispatch::DispatchResult,
    traits::{Get, EnsureOrigin}};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: frame_system::Trait {
    /// In order to provide generality, we need some way to associate some action on a source chain
    /// to some action on a destination chain. This may express tokenX on chain A is equivalent to
    /// tokenY on chain B, or to simply associate that some action performed on chain A should
    /// result in some other action occurring on chain B. ResourceId is defined as a 32 byte array
    /// by ChainSafe.
    type ResourceId: Member + Default + FullCodec + Into<[u8; 32]> + From<[u8; 32]>;
    /// A local mapping of a resource id. Represents anything that a resource id might map to. On
    /// Ethereum, this may be a contract address for transferring assets.
    type Address: Member + Default + FullCodec + Into<[u8; 32]> + From<[u8; 32]>;
    /// Admin is able to set/remove resource mappings.
    type AdminOrigin: EnsureOrigin<Self::Origin>;
}

decl_storage! {
    trait Store for Module<T: Trait> as BridgeMapping {
        /// Indicates that assets of a resource can be transfered to another resource.
        /// Maps an abstract resource id to a chain-specific address
        ResourceToAddress get(fn addr_of): map hasher(blake2_128_concat) T::ResourceId => Option<T::Address>;
        /// Maps a chain-specific address to a resource id. A mapping in [ResourceToAddress] will
        /// always correspond to a mapping here. Resources and addresses are 1 to 1.
        AddressToResource get(fn name_of): map hasher(blake2_128_concat) T::Address => Option<T::ResourceId>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        /// Set a resource mapping in the [Names]. Existing keys will be overwritten.
        /// The caller must be the owner of the `rid` ResourceId.
        #[weight = T::DbWeight::get().reads_writes(0,2) + 100_000]
        pub fn set(origin,
                   rid: T::ResourceId,
                   local_addr: T::Address,
        ) -> DispatchResult {
            Self::ensure_admin_or_root(origin)?;

            // Call internal
            Self::set_resource(rid, local_addr);
            Ok(())
        }

        #[weight = T::DbWeight::get().reads_writes(1,2) + 100_000]
        pub fn remove(origin,
                      rid: T::ResourceId,
        ) -> DispatchResult {
            Self::ensure_admin_or_root(origin)?;

            // Call internal
            Self::remove_resource(&rid);
            Ok(())
        }
    }
}

// Even though the storage structure of this Module does not guarantee that every resource has a
// corresponding owner, the function interfaces defined here ensure this by construction. This
// assumption is something to keep in mind if extending this module.
impl<T: Trait> Module<T> {
    /// Ensure that the given origin is either the pallet [AdminOrigin] or frame_system root.
    fn ensure_admin_or_root(origin: T::Origin) -> Result<(), BadOrigin> {
        T::AdminOrigin::try_origin(origin)
            .map(|_| ())
            .or_else(ensure_root)
    }

    /// Add a new resource mapping in [Names]. Existing entries will be overwritten.
    pub fn set_resource(rid: T::ResourceId,
                        local_addr: T::Address,
    ) {
        // Add the mapping both ways
        ResourceToAddress::<T>::insert(rid.clone(), local_addr.clone());
        AddressToResource::<T>::insert(local_addr, rid);
    }

    /// Remove a resource mapping in [Names].
    pub fn remove_resource(rid: &T::ResourceId) {
        // If it doesn't exist for some unexpected reason, still allow removal by setting default
        let address = ResourceToAddress::<T>::get(rid).unwrap_or_default();

        // Remove the resource mapping both ways
        ResourceToAddress::<T>::remove(rid);
        AddressToResource::<T>::remove(address);
    }
}
