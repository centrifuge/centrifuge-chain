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

//! # Anchors pallet benchmarking routines used for calculating extrinsics weights.
//! 
//! ##Overview
//! As stated in Susbstrate documentation on [benchmarking](https://substrate.dev/docs/en/knowledgebase/runtime/benchmarking),
//! "the time it takes to execute an extrinsic may vary based on the computational complexity, 
//! storage complexity, hardware used, and many other factors. We use generic measurement called 
//! weight to represent how many extrinsics can fit into one block".
//! 
//! ## References
//! [Substrate Benchmarking Documentation](https://www.shawntabrizi.com/substrate-graph-benchmarks/docs/#/)

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

// Import crate components
use super::*;
use crate::{self as pallet_bridge, Pallet as Bridge};

// Import basic benchmarking primitives
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};

// Proofs primitives and processing routines
use proofs::Proof;

// ----------------------------------------------------------------------------
// Benchmarks implementation
// ----------------------------------------------------------------------------

benchmarks! {

    // Benchmark `pre_commit` extrinsic.
    pre_commit_benchmark {

        // Initialize seminal state
    } : {
        // Execute extrinsics
//       pre_commit();
    } verify {
        // Check final state (i.e. verifying that all goes well)
    }

} // end of 'benchmarks' macro


// ----------------------------------------------------------------------------
// Benchmarking test cases generation
// ----------------------------------------------------------------------------

// The following macro generates test cases for benchmarking, and could be ru
// with the following command:
//   `cargo test -p pallet-anchors --all-features`
//
// You will see one line per benchmarking test case, namely:
//   `test benchmarking::pre_commit_benchmark ... ok`
//   ... and so on
//
// The line generates three steps per benchmark, with repeat=1 and the three steps are
// [low, mid, high] of the range.
impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(None),
	crate::mock::MockRuntime,
);

// ----------------------------------------------------------------------------
// Helper functions
// ----------------------------------------------------------------------------

mod helpers {

    // Implement helper functions here

} // end of 'helpers' module