
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

//! # Claim module for crowdloan compaign
//!
//! - \[`Config`]
//! - \[`Call`]
//! - \[`Pallet`]
//!
//! ## Overview
//!
//! This module (or pallet) is used to proces reward claims from Contributors
//! who locked tokens on the Polkadot/Kusama relay chain for participating in
//! a crowdloan campaign for a parachain slot acquisition.
//!
//! This module is intimately bound to the [`crowdloan-reward `] module, where
//! the rewarding strategy (or logic) is implemented.
//! 
//! ## Interface
//!
//! ### Types Declaration
//! ### Dispatchable Functions
//!
//! Callable functions (or extrinsics), also considered as transactions, materialize the module interface
//! (or contract). Here's the callable functions implemented in this module:
//! 
//! ## References
//! [Building a Custom Pallet](https://substrate.dev/docs/en/tutorials/build-a-dapp/pallet). Retrieved April 5th, 2021.


// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use frame_support::{
  decl_error, decl_event, decl_module, decl_storage, 
  traits::{Currency}, 
  weights::{Weight}
};

use sp_runtime::{
  traits::{
    AtLeast32BitUnsigned,
    Hash,
    MaybeSerializeDeserialize,
    Member,
    Parameter,
  }
};

use frame_system::ensure_signed;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Measure the weight of the module's extrinsics (i.e. callable functions)
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;


// ----------------------------------------------------------------------------
// Runtime configuration
// ----------------------------------------------------------------------------

// Runtime types and constants definition.
//
// If the module depends on other pallets, their configuration traits should be
// added to the inherited traits list.
pub trait Config: frame_system::Config {

  /// This module emits events, and hence, depends on the runtime's definition of event
  type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

  /// Weight information for module's dispatchable functions (or extrinsics)
  ///
  /// Weights are calculated using the 
  type WeightInfo: WeightInfo;

  /// Crowdloan module storage trie root
  ///
  /// This type defines a hashed data structure which contains the list
  /// of contributors who participate in the parachain slot auction, as
  /// well as their respective amount of tokens they locked on their 
  /// relay chain account (see RelayChainAccount)
  type CrowdloanStorageTrieRoot: Hash;

  /// Contribution currency type
  ///
  /// The contribution currency depends on the relay chain on which
  /// the contributor locked her/his native tokens.
  type ContributionCurrency: Currency<Self::AccountId>;

  /// Crowdloan contribution amount.
  type ContributionAmount: Parameter
    + Member
    + AtLeast32BitUnsigned
    + Default
    + Copy
    + MaybeSerializeDeserialize
    + Ord;

  /// Contributor's relay chain account type.
  ///
  /// In order to participate to a crowdloan campaign, the contributor
  /// locks native tokens on a relay chain account. For forgoing staking,
  /// a reward is then paid out on a [`ContributorParachainAccountId`].
  type ContributorRelayChainAccountId: Self::AccountId;
}

/// Callable functions (i.e. transaction) weight trait
///
/// See https://substrate.dev/docs/en/knowledgebase/learn-substrate/weight
/// See https://substrate.dev/docs/en/knowledgebase/runtime/fees
pub trait WeightInfo {
  fn initialize() -> Weight;
	fn claim_reward() -> Weight;
  //fn verify_contributor() -> Weight;
}

// Define transaction weights to be used when testing
pub struct TestWeightInfo;
impl WeightInfo for TestWeightInfo {
  fn initialize() -> Weight { 0 }
	fn claim_reward() -> Weight { 0 }
  // fn verify_contributor() -> Weight { 0 }
}


// ----------------------------------------------------------------------------
// Runtime events
// ----------------------------------------------------------------------------

// Events are a simple means of reporting specific conditions and circumstances
// that have happened that users, Dapps and/or chain explorers would find
// interesting and otherwise difficult to detect.
// Events can be used, for instance, to provide the module with life-cycle hooks
// other components can bind to.
//
// See https://substrate.dev/docs/en/knowledgebase/runtime/events
decl_event! { 

  pub enum Event<T> where
		AccountId = <T as frame_system::Config>::AccountId,
    ContributorRelayChainAccountId = <T as frame_system::Config>::ContributorRelayChainAccountId
	{
    /// Event emitted when a reward claim was processed successfully
    RewardClaimed(AccountId, ContributorRelayChainAccountId),

    /// Event triggered when the list of contributors is successfully uploaded
    ClaimModuleInitialized(AccountId),

    /// Contributor who claimed a reward is not the owner of the rewarding account on the parachain
    NotOwnerOfContributorAccount(AccountId),
  }
}


// ----------------------------------------------------------------------------
// Runtime storage
// ----------------------------------------------------------------------------

// This allows for type-safe usage of the Substrate storage database, so you can
// keep things around between blocks.
decl_storage! { 

  trait Store for Module<T: Config> as Claim {

    /// List of contributors and their respective contributions.
    ///
    /// This is a copy of the child trie root stored in the [`crownloan`] module
    /// during the campaign. 
    CrowdloanTrieRoot get(fn crowdloan_trie_root): T::CrowdloanStorageTrieRoot;


    /// List of processed reward claims
    Contributor: map hasher(blake2_128_concat) Vec<u8> => (T::AccountId, bool);
  }
}


// ----------------------------------------------------------------------------
// Module-specific errors
// ----------------------------------------------------------------------------

decl_error! {

	pub enum Error for Module<T: Config> {
    /// Not enough funds in the pot for paying a reward payout
    NotEnoughFunds,
  
    /// Invalid (e.g. malicious replay attack) or malformed reward claim
    InvalidClaim,

    /// Cannot check the contributor's identity
    UnverifiedContributor,
  }
}


// ----------------------------------------------------------------------------
// Dispatchable functions (i.e. module contract)
// ----------------------------------------------------------------------------

decl_module! {

  /// Claim module declaration.
  /// 
  /// This defines the `Module` struct that is ultimately exported from this pallet.
  /// It defines the callable functions that this module exposes and orchestrates
  /// actions this pallet takes throughout block execution.
  /// Dispatchable functions allows users to interact with the pallet and invoke state changes.
  ///
  /// See https://substrate.dev/docs/en/knowledgebase/runtime/macros#decl_module
  pub struct Module<T: Config> for enum Call where origin: T::Origin {

    // Initialize errors
    //
    // It is worth pointing out that callable functions herein do not have a return
    // type explicitely defined. However, they are all returning DispatchResult, that is
    // implicitely added by the `decl_module` macro.
    type Error = Error<T>;

    // Initialize events
    //
    // When used by the module, the events must be first initialized.
    fn deposit_event() = default;

    // Initialize the claim module context
    //
    // # <weight>
    // # </weight>
    #[weight = T::WeightInfo::initialize()]
    fn initialize() {
      Ok(())
    }


    /// Allow a contributor participating in a crowdloan campaign to claim her/his reward payout
    ///
    /// # <weight>
    /// # </weight>
    #[weight = T::WeightInfo::claim_reward()]
    fn claim_reward(origin) {

      // Ensure the extrinsic was signed and get the signer
      // See https://substrate.dev/docs/en/knowledgebase/runtime/origin
      let contributor = ensure_signed(origin)?;

      // Emit an event that the reward claim was processed successfully
      //Self::deposit_event(RawEvent::ClaimCreated(contributor));
      
      Ok(())
    }


    /// Verify the contributor's identity
    ///
    /// Before a contributor can claim a reward payout for the tokens she/he locked on
    /// Polkadot/Kusama relay chain, her/his relay chain and parachain accounts must first
    /// be bound together.
    /// Being in a trustless configuration where the parachain does not know contributors, 
    /// the latter must provide with a proof of their identity. 
    #[weight = T::WeightInfo::verify_contributor()]
    fn verify_contributor() {
      Ok(())
    }
  }
}