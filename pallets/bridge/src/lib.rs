
// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

//! # Bridge pallet
//!
//! This pallet implements bla bla bla
//!
//! - \[`Config`]
//! - \[`Call`]
//! - \[`Pallet`]
//!
//! ## Overview
//! This pallet is used for bridging... bla bla bla
//!
//! ## Terminology
//! 
//! ## Usage
//!
//! ## Interface
//!
//! ### Supported Origins
//!
//! Signed origin is valid.
//!
//! ### Types
//!
//! <code>\`BridgeOrigin\`</code> bla bla bla.
//! <code>\`Currency\`</code> bla bla bla.
//! <code>\`Event\`</code> bla bla bla.
//! <code>\`HashId\`</code> Ids can be defined by the runtime and passed in, perhaps from blake2b_128 hashes.
//! <code>\`NativeTokenId\`</code> bla bla bla.
//!
//! ### Events
//!
//! <code>\`Remark\`</code> bla bla bla.
//!
//! ### Errors
//! <code>\`ResourceIdDoesNotExist\`</code> Resource id provided on initiating a transfer is not a key in bridges-names mapping.
//! <code>\`RegistryIdDoesNotExist\`</code> Registry id provided on receiving a transfer is not a key in bridges-names mapping.
//! <code>\`InvalidTransfer\`</code> bla bla bla
//!
//! ### Dispatchable Functions
//!
//! Callable functions (or extrinsics), also considered as transactions, materialize the
//! pallet contract. Here's the callable functions implemented in this module:
//!
//! [`receive_nonfungible`]
//! [`remark`]
//! [`transfer`]
//! [`transfer_asset`]
//! [`transfer_native`]
//! 
//! ### Public Functions
//!
//! ## Genesis Configuration
//! The pallet is parameterized and configured via [parameter_types] macro, at the time the runtime is built
//! by means of the [`construct_runtime`] macro.
//!
//! ## Related Pallets
//! This pallet is tightly coupled to the following pallets:
//! - Substrate FRAME's [`balances` pallet](https://github.com/paritytech/substrate/tree/master/frame/balances).
//! - Centrifuge Chain [`bridge_mapping` pallet](https://github.com/centrifuge/centrifuge-chain/tree/master/pallets/bridge_mapping).
//! - Centrifuge Chain [`nft` pallet](https://github.com/centrifuge/centrifuge-chain/tree/master/pallets/nft).
//!
//! ## References
//! - [Substrate FRAME v2 attribute macros](https://crates.parity.io/frame_support/attr.pallet.html).
//! 
//! ## Credits
//! The Centrifugians Tribe <tribe@centrifuge.io>

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

// Mock runtime and unit test cases
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Extrinsics weight information
mod weights;

// Runtime, system and frame primitives
use frame_support::{
    dispatch::{
        DispatchResult,
    },
    ensure,
    PalletId,
    traits::{
        Currency,
        EnsureOrigin,
        ExistenceRequirement::KeepAlive,
        Get, 
    }, 
    weights::Weight
};

use frame_system::{
  ensure_root,
};

use sp_runtime::{
    sp_std::{
        convert::TryInto,
        vec::Vec,
    },
    traits::{
        AccountIdConversion,
        CheckedSub,
        Hash,
        SaturatedConversion,
    },
    transaction_validity::{
        InvalidTransaction, 
        TransactionPriority,
        TransactionSource,
        TransactionValidity, 
        ValidTransaction, 
    },
};

use sp_core::{Encode};

//use sp_std::convert::TryInto;

use centrifuge_runtime::va_registry::types::{
    AssetId,
    RegistryId, 
    TokenId
};

use pallet_fees;

use centrifuge_runtime::{
    constants::currency,
    va_registry::types::{
        AssetId,
        RegistryId, 
        TokenId
    },
};

// Extrinsics weight information
pub use crate::traits::WeightInfo as PalletWeightInfo;


// Re-export in crate namespace (for runtime construction)
pub use pallet::*;


// ----------------------------------------------------------------------------
// Traits and types declaration
// ----------------------------------------------------------------------------

pub mod traits {

    use super::*;
    
    /// Weight information for pallet extrinsics
    ///
    /// Weights are calculated using runtime benchmarking features.
    /// See [`benchmarking`] module for more information. 
    pub trait WeightInfo {
        fn receive_nonfungible() -> Weight;
        fn remark() -> Weight;
        fn transfer() -> Weight;
        fn transfer_asset() -> Weight;
        fn transfer_native() -> Weight;
    }
} // end of 'traits' module

/// Abstract identifer of an asset, for a common vocabulary across chains.
pub type ResourceId = chainbridge::ResourceId;

/// A generic representation of a local address. A resource id points to this. It may be a
/// registry id (20 bytes) or a fungible asset type (in the future). Constrained to 32 bytes just
/// as an upper bound to store efficiently.
#[derive(Encode, Clone, PartialEq, Eq, Default, Debug)]
pub struct Address(pub Bytes32);

/// Length of an [Address] type
const ADDR_LEN: usize = 32;

type Bytes32 = [u8; ADDR_LEN];

impl From<RegistryId> for Address {
    fn from(r: RegistryId) -> Self {
        // Pad 12 bytes to the registry id - total 32 bytes
        let padded = r.to_fixed_bytes().iter().copied()
            .chain([0; 12].iter().copied()).collect::<Vec<u8>>()[..ADDR_LEN]
            .try_into().expect("RegistryId is 20 bytes. 12 are padded. Converting to a 32 byte array should never fail");

        Address( padded )
    }
}

// In order to be generic into T::Address
impl From<Bytes32> for Address {
    fn from(v: Bytes32) -> Self {
        Address( v[..ADDR_LEN].try_into().expect("Address wraps a 32 byte array") )
    }
}

impl From<Address> for Bytes32 {
    fn from(a: Address) -> Self {
        a.0
    }
}

type BalanceOf<T> =
    <<T as Trait>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Additional Fee charged when moving native tokens to target chains (RAD)
const TOKEN_FEE: u128 = 20 * currency::RAD;


// ----------------------------------------------------------------------------
// Pallet module
// ----------------------------------------------------------------------------

// Bridge pallet module
//
// The name of the pallet is provided by `construct_runtime` and is used as
// the unique identifier for the pallet's storage. It is not defined in the 
// pallet itself.
#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    // Bridge pallet type declaration.
    //
    // This structure is a placeholder for traits and functions implementation
    // for the pallet.
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);


    // ------------------------------------------------------------------------
    // Pallet configuration
    // ------------------------------------------------------------------------

    /// Bridge pallet's configuration trait.
    ///
    /// Associated types and constants are declared in this trait. If the pallet
    /// depends on other super-traits, the latter must be added to this trait, 
    /// such as, in this case, [`frame_system::Config`] and [`pallet_balances::Config`]
    /// super-traits. Note that [`frame_system::Config`] must always be included.
    #[pallet::config]
    pub trait Config: frame_system::Config 
        + pallet_fees::Config
        + pallet_balances::Config
        + chainbridge::Config
        + pallet_nft::Config
        + pallet_bridge_mapping::Config
    {
        /// Specifies the origin check provided by the chainbridge for calls
        /// that can only be called by the chainbridge pallet.
        type BridgeOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;

        /// Expected currency of the reward claim.
        type Currency: Currency<Self::AccountId>;

        /// Associated type for Event enum
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Ids can be defined by the runtime and passed in, perhaps from blake2b_128 hashes.
        #[pallet::constant]
        type HashId: Get<ResourceId>;

        #[pallet::constant]
        type NativeTokenId: Get<ResourceId>;

        // TODO: move from genesis config
        type Chains: Get<Vec<u8>>;

        type Relayers: Get<Vec<T::AccountId>>;

        type Resources: Get<Vec<(ResourceId, Vec<u8>)>>;
        
        type Threshold: Get<u32>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: PalletWeightInfo;
    }
  

    // ------------------------------------------------------------------------
    // Pallet events
    // ------------------------------------------------------------------------

    // The macro generates event metadata and derive Clone, Debug, Eq, PartialEq and Codec
    #[pallet::event]
    // The macro generates a function on Pallet to deposit an event
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {

        /// TODO... better comment
        Remark(<T as frame_system::Config>::Hash, ResourceId),
    }


    // ------------------------------------------------------------------------
    // Pallet storage items
    // ------------------------------------------------------------------------

    // No storage items
    
    
    // ------------------------------------------------------------------------
    // Pallet genesis configuration
    // ------------------------------------------------------------------------

	// The genesis configuration type.
	#[pallet::genesis_config]
	pub struct GenesisConfig {
    }

	// The default value for the genesis config type.
	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self {
                // nothing to do folks!!!!
            }
		}
	}

	// The build of genesis for the pallet.
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
            // nothing to do folks... let's use 'parameters_type' macro instead!!!!
		}
	}


    // ----------------------------------------------------------------------------
    // Pallet lifecycle hooks
    // ----------------------------------------------------------------------------
    
    #[pallet::hooks]
	impl<T:Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}


    // ------------------------------------------------------------------------
    // Pallet errors
    // ------------------------------------------------------------------------

    #[pallet::error]
	pub enum Error<T> {

        /// Resource id provided on initiating a transfer is not a key in bridges-names mapping.
        ResourceIdDoesNotExist,

        /// Registry id provided on recieving a transfer is not a key in bridges-names mapping.
        RegistryIdDoesNotExist,
        
        /// Invalid transfer
        InvalidTransfer,
    }


    // ------------------------------------------------------------------------
    // Pallet dispatchable functions
    // ------------------------------------------------------------------------

    // Declare Call struct and implement dispatchable (or callable) functions.
    //
    // Dispatchable functions are transactions modifying the state of the chain. They
    // are also called extrinsics are constitute the pallet's public interface.
    // Note that each parameter used in functions must implement `Clone`, `Debug`, 
    // `Eq`, `PartialEq` and `Codec` traits.
    #[pallet::call]
	impl<T:Config> Pallet<T> {

        /// Transfer an nft to a whitelisted destination chain. Source nft is locked in bridge account
        /// rather than being burned.
        #[pallet::weight(<T as Config>::WeightInfo::transfer_asset())]
        pub fn transfer_asset(
            origin: OriginFor<T>,
            recipient: Vec<u8>,
            from_registry: RegistryId,
            token_id: TokenId,
            dest_id: chainbridge::ChainId,
        ) -> DispatchResultWithPostInfo {
            let source = ensure_signed(origin)?;

            // Get resource id from registry
            let reg: Address = from_registry.into();
            let reg: Bytes32 = reg.into();
            let reg: <T as bridge_mapping::Config>::Address = reg.into();
            let resource_id = <bridge_mapping::Pallet<T>>::name_of(reg)
                .ok_or(Error::<T>::ResourceIdDoesNotExist)?;

            // Burn additional fees
            let nft_fee: T::Balance = TOKEN_FEE.saturated_into();
            <pallet_fees::Pallet<T>>::burn_fee(&source, nft_fee)?;

            // Lock asset by transfering to bridge account
            let bridge_id = <chainbridge::Module<T>>::account_id();
            let asset_id = AssetId(from_registry, token_id);
            <nft::Module<T> as Unique>::transfer(&source, &bridge_id, &asset_id)?;

            // Transfer instructions for relayer
            let tid: &mut [u8] = &mut[0; 32];
            // Ethereum is big-endian
            token_id.to_big_endian(tid);
            <chainbridge::Pallet<T>>::transfer_nonfungible(
                dest_id,
                resource_id.into(),
                tid.to_vec(),
                recipient,
                vec![]/*assetinfo.metadata*/)
        }


        /// Transfers some amount of the native token to some recipient on a (whitelisted) destination chain.
        #[pallet::weight(<T as Config>::WeightInfo::transfer_native())]
        pub fn transfer_native(
            origin: OriginFor<T>,
            amount: BalanceOf<T>, 
            recipient: Vec<u8>, 
            dest_id: chainbridge::ChainId
        ) -> DispatchResultWithPostInfo {
            let source = ensure_signed(origin)?;

            let token_fee: T::Balance = TOKEN_FEE.saturated_into();
			let total_amount = U256::from(amount.saturated_into()).saturating_add(U256::from(token_fee.saturated_into()));

            // Ensure account has enough balance for both fee and transfer
            // Check to avoid balance errors down the line that leave balance storage in an inconsistent state
            let current_balance = T::Currency::free_balance(&source);
            ensure!(U256::from(current_balance.saturated_into()) >= total_amount, "Insufficient Balance");

            ensure!(<chainbridge::Module<T>>::chain_whitelisted(dest_id), Error::<T>::InvalidTransfer);

            // Burn additional fees
            <pallet_fees::Pallet<T>>::burn_fee(&source, token_fee)?;

            let bridge_id = <chainbridge::Module<T>>::account_id();
            T::Currency::transfer(&source, &bridge_id, amount.into(), AllowDeath)?;

            let resource_id = T::NativeTokenId::get();
            <chainbridge::Module<T>>::transfer_fungible(dest_id, resource_id, recipient, U256::from(amount.saturated_into()))?;

            Ok(().into())
        }

        /// Executes a simple currency transfer using the chainbridge account as the source
        #[pallet::weight(<T as Config>::WeightInfo::transfer())]
        pub fn transfer(
            origin: OriginFor<T>,
            to: T::AccountId,
            amount: BalanceOf<T>,
            r_id: ResourceId
        ) -> DispatchResultWithPostInfo {
            let source = T::BridgeOrigin::ensure_origin(origin)?;
            T::Currency::transfer(&source, &to, amount.into(), AllowDeath)?;

            Ok(().into())
        }
   
        #[pallet::weight(<T as Config>::WeightInfo::receive_nonfungible())]
        pub fn receive_nonfungible(
            origin: OriginFor<T>,
            to: T::AccountId,
            token_id: TokenId,
            _metadata: Vec<u8>,
            resource_id: ResourceId
        ) -> DispatchResultWithPostInfo {
            let source = T::BridgeOrigin::ensure_origin(origin)?;

            // Get registry from resource id
            let rid: <T as bridge_mapping::Config>::ResourceId = resource_id.into();
            let registry_id = <bridge_mapping::Module<T>>::addr_of(rid)
                .ok_or(Error::<T>::RegistryIdDoesNotExist)?;
            let registry_id: Address = registry_id.into().into();

            // Transfer from bridge account to destination account
            let asset_id = AssetId(registry_id.into(), token_id);
            <nft::Module<T> as Unique>::transfer(&source, &to, &asset_id)
        }
   
        /// This can be called by the chainbridge to demonstrate an arbitrary call from a proposal.
        #[pallet::weight(<T as Config>::WeightInfo::remark())]
        pub fn remark(
            origin: OriginFor<T>,
            hash: T::Hash, 
            r_id: ResourceId
        ) -> DispatchResultWithPostInfo {
            T::BridgeOrigin::ensure_origin(origin)?;
            Self::deposit_event(RawEvent::Remark(hash, r_id));

            Ok(().into())
        }
    }
} // end of 'pallet' module


// ----------------------------------------------------------------------------
// Pallet implementation block
// ----------------------------------------------------------------------------

// Pallet implementation block.
//
// This main implementation block contains two categories of functions, namely:
// - Public functions: These are functions that are `pub` and generally fall into 
//   inspector functions that do not write to storage and operation functions that do.
// - Private functions: These are private helpers or utilities that cannot be called 
//   from other pallets.
impl<T: Config> Pallet<T> {
   
    /// Return the account identifier of the RAD claims pallet.
	///
	/// This actually does computation. If you need to keep using it, then make
	/// sure you cache the value and only call this once.
	pub fn account_id() -> T::AccountId {
	  T::PalletId::get().into_account()
	}
}
