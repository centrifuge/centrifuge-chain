
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
//! ## Overview
//!
//! This module (or pallet) is used to manage the reward claims made by Contributors
//! who locked tokens on the Polkadot/Kusama relay chain for their participation in
//! a crowdloan campaign.
//!
//! This module is intimately bound to the crowdloan-reward pallet, where the rewarding
//! strategy (or logic) is implemented.
//! 
//! ## Callable functions
//!
//! 
//! ## References
//! [Building a Custom Pallet](https://substrate.dev/docs/en/tutorials/build-a-dapp/pallet). Retrieved April 5th, 2021.


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
  decl_module, 
  decl_storage, 
  decl_event, 
  decl_error, 
  dispatch,
  pallet_prelude::{
    Weight
  },
};

use frame_system::ensure_signed;

/// Callable functions' weight trait
pub trait WeightInfo {
	fn create() -> Weight;
}

pub struct TestWeightInfo;
impl WeightInfo for TestWeightInfo {
	fn create() -> Weight { 0 }
	fn contribute() -> Weight { 0 }
	fn withdraw() -> Weight { 0 }
	fn refund(_k: u32, ) -> Weight { 0 }
	fn dissolve() -> Weight { 0 }
	fn edit() -> Weight { 0 }
	fn add_memo() -> Weight { 0 }
	fn on_initialize(_n: u32, ) -> Weight { 0 }
	fn poke() -> Weight { 0 }
}


// ----------------------------------------------------------------------------
// Runtime configuration
// ----------------------------------------------------------------------------

// All of the runtime types and consts go in here. If the pallet
// is dependent on specific other pallets, then their configuration traits
// should be added to the inherited traits list.
pub trait Config: frame_system::Config { 

  /// This module emits events, and hence, depends on the runtime's definition of event
  type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
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
decl_event!{ 

  /// Event emitted when a proof has been claimed. [who, claim]
  ClaimCreated(AccountId, Vec<u8>),

  /// Event emitted when a claim is successfully processed
  ClaimProcessed(AccointId)
}


// ----------------------------------------------------------------------------
// Runtime storage
// ----------------------------------------------------------------------------

// This allows for type-safe usage of the Substrate storage database, so you can
// keep things around between blocks.
decl_storage! { 
  
}


// ----------------------------------------------------------------------------
// Module-specific errors
// ----------------------------------------------------------------------------

decl_error! {
	pub enum Error for Module<T: Config> {
  /// Not enough funds for a reward payout
  NotEnoughFunds,
  
  /// Invalid (e.g. malicious replay attack) or malformed reward claim
  InvalidClaim
}


// ----------------------------------------------------------------------------
// Dispatchable functions (or contract)
// ----------------------------------------------------------------------------

// This defines the `Module` struct that is ultimately exported from this pallet.
// It defines the callable functions that this pallet exposes and orchestrates
// actions this pallet takes throughout block execution.
// Callable (or dispatchable) functions are like extrinsics, but are often considered
// as transactions.
decl_module! {

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

  /// Allow a contributor participating in a crowdloan campaign to claim for her/his reward payout
  #[weight = T::WeightInfo::claim_reward(T::RemoveKeysLimit::get())]
  fn claim_reward_payout(origin) {

    // Emit an event that the reward claim was processed successfully
    Self::claim_event(RawEvent::ClaimCreated(sender));
  }
}