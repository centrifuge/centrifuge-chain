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

// # Crowdloan reward pallet's transactions benchmarking
//
// ## Overview
// A limited number of extrinsics can be executed per block. In fact, Substrate
// produces blocks at a regular intervals, that limits the number of extrinsics
// that can be executed per block. A generic measurement, called a **weight** is
// used to figure out how many extrinsics a block can support.
//
// This benchmarking module uses the Substrate Runtime Benchmaking (SRB) tool
// to automate the computation of the weight of each extrinsics implemented in
// this pallet. The result is an output Rust file which contains a handy function
// for getting weights of extrinsics and that can be easily integrated to the
// chain's runtime framework.
//
// ## How to Benchmark
// To actually benchmark the pallet, it must be [added to the runtime](https://substrate.dev/docs/en/knowledgebase/runtime/benchmarking#add-benchmarking-to-your-runtime)
// and the runtime compiled with `runtime-benchmarks` feature, as shown below:
//
// ```sh
// $ cargo build --release --features runtime-benchmarks
// ```
//
// The resulting auto-generated weight estimate for the extrinsics implemented in the
// pallet is stored in the `weights.rs` file. The latter must not be modified manually.
// The exact command of how the estimate was generated, is printed at the top of this file.
//
// ## References
// - [Building a Custom Pallet](https://substrate.dev/docs/en/tutorials/build-a-dapp/pallet). Retrieved April 5th, 2021.
// - [Runtime Benchmarking](https://substrate.dev/docs/en/knowledgebase/runtime/benchmarking). Retrieved April 10th, 2021.
// - [Benchmarking Macros](https://substrate.dev/rustdocs/v3.0.0/frame_benchmarking/macro.benchmarks.html)

#![cfg(feature = "runtime-benchmarks")]

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

use crate::*;

// ----------------------------------------------------------------------------
// Benchmark cases
// ----------------------------------------------------------------------------

benchmarks! {
git sample_benchmark_name {
        // setup initial state
    }: {
        // benchmark code
    } verify {
        // verifying final state
    }
}

// ----------------------------------------------------------------------------
// Benchmark tests
// ----------------------------------------------------------------------------

// Generate test cases for benchmarking.
//
// This macro generates the test cases for benchmarking. It can be executed using
// the following command:
//
// ```sh
// $ cargo test -p pallet-crowdloan-claim --all-features
// ```
//
// At the end of the execution, the following message is printed in the result:
//
// `test benchmarking::benchmark_tests::test_benchmarks ... ok`.
//
// The line generates three steps per benchmark, with repeat=1 and the three steps are
//   [low, mid, high] of the range.
impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test,);
