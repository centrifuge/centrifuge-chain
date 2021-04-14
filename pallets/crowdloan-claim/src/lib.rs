
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
//! This module (or pallet) is used to proces reward claims from Contributors
//! who locked tokens on the Polkadot/Kusama relay chain for participating in
//! a crowdloan campaign.
//!
//! This module is intimately bound to the crowdloan-reward module, where the rewarding
//! strategy (or logic) is implemented.
//! 
//! ## Callable functions
//!
//! Callable functions, also considered as transactions, materialize the module interface
//! (or contract). Here's the callable functions implemented in this module:
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
  weights::{Weight}
};

use frame_system::{ensure_signed, Event};

pub mod reward;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;


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

  /// Weight information for the extrinsics in the module
  type WeightInfo: WeightInfo;
}

/// Callable functions (i.e. transaction) weight trait
///
/// See https://substrate.dev/docs/en/knowledgebase/learn-substrate/weight
/// See https://substrate.dev/docs/en/knowledgebase/runtime/fees
pub trait WeightInfo {
  fn initialize() -> Weight;
	fn claim_reward() -> Weight;
  fn verify_contributor() -> Weight;
}

// Define transaction weights to be used when testing
pub struct TestWeightInfo;
impl WeightInfo for TestWeightInfo {
  fn initialize() -> Weight { 0 }
	fn claim_reward() -> Weight { 0 }
  fn verify_contributor() -> Weight { 0 }
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


// ----------------------------------------------------------------------------
// Runtime storage
// ----------------------------------------------------------------------------

// This allows for type-safe usage of the Substrate storage database, so you can
// keep things around between blocks.


// ----------------------------------------------------------------------------
// Module-specific errors
// ----------------------------------------------------------------------------


// ----------------------------------------------------------------------------
// Callable (dispatchable) functions (i.e. module contract)
// ----------------------------------------------------------------------------

// This defines the `Module` struct that is ultimately exported from this pallet.
// It defines the callable functions that this pallet exposes and orchestrates
// actions this pallet takes throughout block execution.
// Dispatchable functions allows users to interact with the pallet and invoke state changes.
// These functions materialize as "extrinsics", which are often compared to transactions.
// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
//
// See https://substrate.dev/docs/en/knowledgebase/runtime/macros#decl_module