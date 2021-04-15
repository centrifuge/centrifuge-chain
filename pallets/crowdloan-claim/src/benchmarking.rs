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

//! # Crowdloan Claim Module Benchmarking
//!
//! ## Overview
//!
//! A limited number of extrinsics can be executed per block. In fact, Substrate
//! produces blocks at a regular intervals, that limits the number of extrinsics
//! that can be executed per block. A generic measurement, called a **weight** is
//! used to figure out how many extrinsics a block can support.
//!
//! This benchmarking module uses the Substrate Runtime Benchmaking (SRB) tool
//! to automate the computation of the weight of each extrinsics implemented in 
//! this pallet. The result is an output Rust file which contains a handy function
//! for getting weights of extrinsics and that can be easily integrated to the
//! chain's runtime framework.  
//!
//! ## References
//! [Building a Custom Pallet](https://substrate.dev/docs/en/tutorials/build-a-dapp/pallet). Retrieved April 5th, 2021.
//! [Runtime Benchmarking](https://substrate.dev/docs/en/knowledgebase/runtime/benchmarking). Retrieved April 10th, 2021.


#![cfg(feature = "runtime-benchmarks")]


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------
mod benchmarking;
pub mod weights;

use crate::{*, Module as ClaimModule};
use frame_benchmarking::{benchmarks, account, impl_benchmark_test_suite};
use frame_system::RawOrigin;

/// Module we're benchmarking here.
pub struct Module<T: Config<I>, I: Instance>(CrowdloanClaimModule<T, I>);

  benchmarks! {
    benchmark_name {
      // setup initial state
    }: {
      // benchmark code
    } verify {
      /* verifying final state */
    }
  }


  // ----------------------------------------------------------------------------
  // Benchmarking tests
  // ----------------------------------------------------------------------------

  impl_benchmark_test_suite!(
    ClaimModule,
    crate::tests::new_test_ext(),
    crate::tests::Test,
  );
}