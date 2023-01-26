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

//! Module provides benchmarking for Loan Pallet
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};

use super::*;
use crate::test_utils::*;

benchmarks! {
	// Our logarithmic-time pow implementation is effectively
	// linear in the number of bits, or log2(n).
	// This creates a benchmark which takes that number of bits
	// (written in code as `type::NUM_BITS - val.leading_zeroes()`)
	// and returns a reasonably-precise weight for the pow.
	calculate_accumulated_rate {
		let n in 1..25;
		let now: Moment = (1 << n) - 1;
		let rate = interest_rate_per_sec(T::InterestRate::saturating_from_rational(10, 100)).unwrap();
	}: { Pallet::<T>::calculate_accumulated_rate(rate, One::one(), 0, now).unwrap() }
	verify {
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::Runtime,
);
