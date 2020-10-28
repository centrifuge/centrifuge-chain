//! # Bridge Access Control List Pallet
//!
//! This pallet provides functionality for setting and tracking allowed paths for assets to be
//! transferred across chains, or more abstractly, resources. A resource (defined in [ResourceId]
//! has an owner which 
#![cfg_attr(not(feature = "std"), no_std)]

use codec::FullCodec;
use sp_runtime::traits::Member;
use frame_system::ensure_signed;
use frame_support::{
    decl_module, decl_storage, decl_event, decl_error,
    ensure, dispatch::DispatchResult, traits::Get};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: frame_system::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// In order to provide generality, we need some way to associate some action on a source chain
    /// to some action on a destination chain. This may express tokenX on chain A is equivalent to
    /// tokenY on chain B, or to simply associate that some action performed on chain A should
    /// result in some other action occurring on chain B. All resource ids are considered to have a
    /// home chain. For instance, on Ethereum, a resource id may encode the address to a mint
    /// method of a contract.
    type ResourceId: Member + Default + FullCodec;
}

decl_storage! {
    trait Store for Module<T: Trait> as BridgeACL {
        /// Indicates that assets of a resource can be transfered to another resource.
        ACL: map hasher(blake2_128_concat) T::ResourceId => T::ResourceId;
        /// Maps an owner of a resource.
        Owner get(fn owner_of): map hasher(blake2_128_concat) T::ResourceId => T::AccountId;
    }
}

decl_event!(
    pub enum Event<T> where AccountId = <T as frame_system::Trait>::AccountId {
        SomethingStored(u32, AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// The caller does not own the resource they are trying to modify.
        NotOwnerOfResource,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        /// Set a resource mapping in the [ACL]. Existing keys will be overwritten.
        /// The caller must be the owner of the `from` ResourceId.
        #[weight = 195_000_000]
        pub fn set(origin,
                   from: T::ResourceId,
                   to: T::ResourceId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Call internal
            Self::set_resource(&caller, from, to)
        }

        #[weight = 195_000_000]
        pub fn remove(origin,
                      from: T::ResourceId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Call internal
            Self::remove_resource(&caller, &from)
        }
    }
}

// Even though the storage structure of this Module does not guarantee that every resource has a
// corresponding owner, the function interfaces defined here ensure this by construction. This
// assumption is something to keep in mind if extending this module.
impl<T: Trait> Module<T> {
    /// Update a resource mapping in the [ACL]. Existing keys will be overwritten.
    /// The caller must be the owner of the `from` ResourceId.
    pub fn set_resource(caller: &T::AccountId,
                        from: T::ResourceId,
                        to: T::ResourceId,
    ) -> DispatchResult {
        // Caller is owner of resource
        ensure!(caller == &Self::owner_of(&from),
                Error::<T>::NotOwnerOfResource);

        // Add the mapping
        ACL::<T>::insert(from, to);
        Ok(())
    }

    /// Add a resource mapping in the [ACL], and set its owner.
    /// Existing keys will be overwritten.
    pub fn add_resource(owner: T::AccountId,
                        from: T::ResourceId,
                        to: T::ResourceId,
    ) {
        // Add the mapping
        ACL::<T>::insert(from, to.clone());
        // Set the owner
        Self::set_owner(owner, to);
    }

    /// Remove a resource mapping in the [ACL]. The caller must be the owner of the resouce.
    pub fn remove_resource(caller: &T::AccountId,
                           from: &T::ResourceId,
    ) -> DispatchResult {
        // Caller is owner of resource
        ensure!(caller == &Self::owner_of(from),
                Error::<T>::NotOwnerOfResource);

        // Remove the resource mapping
        ACL::<T>::remove(from);
        Ok(())
    }

    /// Update the [Owner] mapping. If an existing key is provided, it will override the value.
    pub fn set_owner(owner: T::AccountId,
                     resource: T::ResourceId
    ) {
        Owner::<T>::insert(resource, owner);
    }
}
