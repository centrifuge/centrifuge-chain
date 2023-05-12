// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Offer utilities to create custom pallet mocks for generic traits.
//!
//! [`register_call!()`] and [`execute_call!()`] expect the following storage in
//! the pallet. It's safe to just copy and paste in your pallet mock.
//!
//! ```no_run
//! # #[frame_support::pallet]
//! # mod pallet {
//! # use frame_support::pallet_prelude::*;
//! # #[pallet::config]
//! # pub trait Config: frame_system::Config { }
//! # #[pallet::pallet]
//! # #[pallet::generate_store(pub(super) trait Store)]
//! # pub struct Pallet<T>(_);
//!
//! #[pallet::storage]
//! pub(super) type CallIds<T: Config> = StorageMap<
//!     _,
//!     Blake2_128Concat,
//!     <Blake2_128 as frame_support::StorageHasher>::Output,
//!     mock_builder::CallId,
//! >;
//!
//! # }
//! ```
//!
//! Take a look to the [pallet tests](`tests/pallet.rs`) to have a user view of
//! how to use this crate.

/// Provide functions for register/execute calls
pub mod storage;

/// Provide functions for handle fuction locations
pub mod location;

mod util;

use frame_support::{Blake2_128, StorageHasher, StorageMap};
use location::FunctionLocation;
pub use storage::CallId;

/// Prefix that the register functions should have.
pub const MOCK_FN_PREFIX: &str = "mock_";

/// Register a mock function into the mock function storage.
/// This function should be called with a locator used as a function
/// identification.
pub fn register<Map, L, F, I, O>(locator: L, f: F)
where
	Map: StorageMap<<Blake2_128 as StorageHasher>::Output, CallId>,
	L: Fn(),
	F: Fn(I) -> O + 'static,
{
	let location = FunctionLocation::from(locator)
		.normalize()
		.strip_name_prefix(MOCK_FN_PREFIX)
		.append_type_signature::<I, O>();

	Map::insert(location.hash::<Blake2_128>(), storage::register_call(f));
}

/// Execute a function from the function storage.
/// This function should be called with a locator used as a function
/// identification.
pub fn execute<Map, L, I, O>(locator: L, input: I) -> O
where
	Map: StorageMap<<Blake2_128 as StorageHasher>::Output, CallId>,
	L: Fn(),
{
	let location = FunctionLocation::from(locator)
		.normalize()
		.append_type_signature::<I, O>();

	let call_id = Map::try_get(location.hash::<Blake2_128>())
		.unwrap_or_else(|_| panic!("Mock was not found. Location: {location:?}"));

	storage::execute_call(call_id, input).unwrap_or_else(|err| {
		panic!("{err}. Location: {location:?}");
	})
}

/// Register a mock function into the mock function storage.
/// Same as `register()` but with using the locator who calls this macro.
#[macro_export]
macro_rules! register_call {
	($f:expr) => {{
		$crate::register::<CallIds<T>, _, _, _, _>(|| (), $f)
	}};
}

/// Execute a function from the function storage.
/// Same as `execute()` but with using the locator who calls this macro.
#[macro_export]
macro_rules! execute_call {
	($input:expr) => {{
		$crate::execute::<CallIds<T>, _, _, _>(|| (), $input)
	}};
}
