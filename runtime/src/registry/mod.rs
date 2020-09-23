//! This substrate pallet defines a Verifiable Attributes Registry
//! for minting and managing non-fungible tokens (NFT). A registry
//! can be treated like a class of NFTs, where each class can define
//! unique minting and burning logic upon creation at runtime.
//!
//! There are many ways to define a registry, and this pallet abstracts
//! the notion of a registry into a trait called [VerifierRegistry].
//!
//! In particular, upon creation the VA Registry is supplied with a list
//! of data field names from the fields attribute of the [RegistryInfo]
//! struct. Values for the fields are provided upon each call to
//! [mint](struct.Module.html#method.mint) a new NFT. As can be seen in
//! the values field of the [MintInfo] struct. MintInfo also takes a
//! list of proofs and an anchor id. The mint method will hash the
//! values into leaves of a merkle tree and aggregate with the proofs
//! to generate the root. When the root hash matches that of the anchor,
//! a mint can be verified.

//#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_module, decl_storage, decl_event, decl_error,
    ensure, dispatch};
use frame_system::ensure_signed;
use sp_std::{vec::Vec, cmp::Eq};
use crate::nft::InRegistry;
use unique_assets::traits::{Unique, Nft, Mintable};
pub use types::{*, VerifierRegistry};
use sp_runtime::traits::Hash;
use crate::nft;

// TODO:
//- Fix unit tests for pallet_nft
//- Write tests for transfer and burn dispatchables in va-registry
//- Integrate bridge pallet
//- Review spec, compare with implementation
//- Figure abstractions for nft macro
//- Consider weights for mint, burn, and transfer

// Types for this module
mod types;

// TODO: tmp until integrated w/ cent chain
mod proofs;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;


pub trait Trait: frame_system::Trait + nft::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as VARegistry {
        /// This is a dummy store for testing verification in template node.
        Anchor get(fn get_anchor_by_id): map hasher(identity) T::Hash => Option<T::Hash>;
        /// Nonce for generating new registry ids.
        RegistryNonce: RegistryId;
        /// A mapping of all created registries and their metadata.
        Registries: map hasher(blake2_128_concat) RegistryId => RegistryInfo;
        /// A list of asset ids for each registry.
        // TODO: Try a map of BTreeSets as well, and do a benchmark comparison
        NftLists: double_map hasher(identity) RegistryId, hasher(identity) AssetId<T> => ();
    }
}

decl_event!(
    pub enum Event<T>
    where
        CommodityId = AssetId<T>,
        AccountId   = <T as frame_system::Trait>::AccountId,
    {
        /// Successful mint of an NFT from fn [`mint`](struct.Module.html#method.mint)
        Mint(CommodityId),
        /// Successful creation of a registry from fn
        /// [`create_registry`](./struct.Module.html#method.create_registry)
        RegistryCreated(RegistryId),
        /// Ownership of the commodity has been transferred to the account.
        Transferred(CommodityId, AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// TODO: Tmp until using anchor pallet
        DocumentNotAnchored,
        /// A specified registry is not in the module storage Registries map.
        RegistryDoesNotExist,
        /// The values vector provided to a mint call doesn't match the length of the specified
        /// registry's fields vector.
        InvalidMintingValues,
        // Thrown when someone who is not the owner of a commodity attempts to transfer or burn it.
        NotCommodityOwner,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        #[weight = 10_000]
        pub fn tmp_set_anchor(origin, anchor_id: T::Hash, anchor: T::Hash
        ) -> dispatch::DispatchResult {
            Ok(<Anchor<T>>::insert(anchor_id, anchor))
        }

        #[weight = 10_000]
        pub fn create_registry(origin,
                               info: RegistryInfo,
        ) -> dispatch::DispatchResult {
            ensure_signed(origin)?;

            let registry_id = <Self as VerifierRegistry>::create_registry(&info)?;

            // Emit event
            Self::deposit_event(Event::<T>::RegistryCreated(registry_id));

            Ok(())
        }

        #[weight = 10_000]
        pub fn mint(origin,
                    owner_account: <T as frame_system::Trait>::AccountId,
                    commodity_info: T::CommodityInfo,
                    mint_info: MintInfo<<T as frame_system::Trait>::Hash>,
        ) -> dispatch::DispatchResult {
            ensure_signed(origin)?;

            // Internal mint validates proofs and modifies state or returns error
            let commodity_id = <Self as VerifierRegistry>::mint(owner_account,
                                                                commodity_info,
                                                                mint_info)?;

            // Mint event
            Self::deposit_event(RawEvent::Mint(commodity_id));

            Ok(())
       }

        /// Transfer a commodity to a new owner.
        ///
        /// The dispatch origin for this call must be the commodity owner.
        ///
        /// This function will throw an error if the new owner already owns the maximum
        /// number of this type of commodity.
        ///
        /// - `dest_account`: Receiver of the commodity.
        /// - `commodity_id`: The hash (calculated by the runtime system's hashing algorithm)
        ///   of the info that defines the commodity to destroy.
        #[weight = 10_000]
        pub fn transfer(origin, dest_account: T::AccountId, commodity_id: AssetId<T>) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(who == <nft::Module<T>>::account_for_commodity(&commodity_id), Error::<T>::NotCommodityOwner);

            <nft::Module<T> as Unique>::transfer(&dest_account, &commodity_id)?;
            Self::deposit_event(RawEvent::Transferred(commodity_id.clone(), dest_account.clone()));
            Ok(())
        }
    }
}

// Auxillary methods of the module for internal use
impl<T: Trait> Module<T> {
    fn get_document_root(anchor_id: T::Hash) -> Result<T::Hash, dispatch::DispatchError> {
        //match <anchor::Module<T>>::get_anchor_by_id(*anchor_id) {
        match Self::get_anchor_by_id(anchor_id) {
            //Some(anchor_data) => Ok(anchor_data.doc_root),
            Some(anchor_data) => Ok(anchor_data),
            None => Err(Error::<T>::DocumentNotAnchored.into()),
        }
    }

    fn create_new_registry_id() -> Result<RegistryId, dispatch::DispatchError> {
        let id = <RegistryNonce>::get();

        // Check for overflow on index
        let nplus1 = <RegistryNonce>::get().checked_add(1)
            .ok_or("Overflow when updating registry nonce.")?;

        // Update the nonce
        <RegistryNonce>::put( nplus1 );

        Ok(id)
    }

    /// Generates a hash of the concatenated inputs, consuming inputs in the process.
    fn leaf_hash(mut field: bytes, mut value: bytes/*, salt: u32*/) -> T::Hash {
        // Generate leaf hash from field ++ value
        let leaf_data = field.extend(value);
        <T as frame_system::Trait>::Hashing::hash_of(&leaf_data)
    }
}

// Implement the verifier registry. This module verifies data fields that are custom defined
// by a registry and provided in the MintInfo during a mint invocation.
impl<T: Trait> VerifierRegistry for Module<T> {
    type AccountId    = <T as frame_system::Trait>::AccountId;
    type RegistryId   = RegistryId;
    type RegistryInfo = RegistryInfo;
    type AssetId      = AssetId<T>;
    type AssetInfo    = <T as nft::Trait>::CommodityInfo;
    type MintInfo     = MintInfo<<T as frame_system::Trait>::Hash>;

    // Registries with identical RegistryInfo may exist
    fn create_registry(info: &Self::RegistryInfo) -> Result<Self::RegistryId, dispatch::DispatchError> {
        // Generate registry id as nonce
        let id = Self::create_new_registry_id()?;

        // Insert registry in storage
        Registries::insert(id.clone(), info);

        Ok(id)
    }

    fn mint(owner_account: <T as frame_system::Trait>::AccountId,
            commodity_info: T::CommodityInfo,
            mint_info: MintInfo<<T as frame_system::Trait>::Hash>,
    ) -> Result<Self::AssetId, dispatch::DispatchError> {
        let registry_id = commodity_info.registry_id();
        let registry_info = Registries::get(registry_id);

        // Check that the registry exists
        ensure!(
            // TODO: Use the decl above
            Registries::contains_key(registry_id),
            Error::<T>::RegistryDoesNotExist
        );

        let fields = registry_info.fields;
        let values = mint_info.values;
        // The number of values passed in should match the number of fields for the registry
        ensure!(
            fields.len() == values.len(),
            Error::<T>::InvalidMintingValues
        );

        // -------------
        // Verify proofs

        // Get the doc root
        // TODO: Use this line instead, once integrated with cent chain
        // let anchor_data = <anchor::Module<T>>::get_anchor_by_id(anchor_id).ok_or("Anchor doesn't exist")?;
        /*** Tmp replacement for tests: ***/
        let doc_root = Self::get_document_root(mint_info.anchor_id)?;

        // Generate leaf hashes of each value for proof
        let leaves = fields.into_iter()
            .zip(values)
            .map(|(field, val)|
                Self::leaf_hash(field, val));

        // Verify the proof against document root
        // TODO: Once integrated w/ cent chain
        //Self::validate_proofs(&doc_root, &proofs, &static_proofs)?;

        // -------
        // Minting

        // Internal nft mint
        let commodity_id = <nft::Module<T>>::mint(&owner_account, commodity_info)?;

        // Place asset id in registry map
        NftLists::<T>::insert(registry_id, commodity_id, ());

        Ok(commodity_id)
    }
}
