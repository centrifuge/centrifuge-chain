// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// //! Benchmarking setup for pallet-template
// #![cfg(feature = "runtime-benchmarks")]
// use super::*;

// #[allow(unused)]
// use crate::Pallet as Template;
// use frame_benchmarking::v2::*;
// use frame_system::RawOrigin;

// #[benchmarks]
// mod benchmarks {
// 	use super::*;

// 	#[benchmark]
// 	fn do_something() {
// 		let value = 100u32.into();
// 		let caller: T::AccountId = whitelisted_caller();
// 		#[extrinsic_call]
// 		do_something(RawOrigin::Signed(caller), value);

// 		assert_eq!(Something::<T>::get(), Some(value));
// 	}

// 	#[benchmark]
// 	fn cause_error() {
// 		Something::<T>::put(100u32);
// 		let caller: T::AccountId = whitelisted_caller();
// 		#[extrinsic_call]
// 		cause_error(RawOrigin::Signed(caller));

// 		assert_eq!(Something::<T>::get(), Some(101u32));
// 	}

// 	impl_benchmark_test_suite!(Template, crate::mock::new_test_ext(), crate::mock::Test);
// }
