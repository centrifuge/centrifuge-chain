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

//! `mock-builder` allows you to create *mock pallets*.
//! A *mock pallet* is a regular pallet that implements some traits whose
//! behavior can be implemented on the fly by closures. They are perfect for
//! testing because they allow you to customize each test case, getting
//! organized and accurate tests for your pallet. *Mock pallet* is not just a
//! trait mocked, it's a whole pallet that can implement one or more traits and
//! can be added to runtimes.
//!
//! # Motivation
//!
//! Pallets have dependencies. Programming in a
//! [loosely coupled](https://docs.substrate.io/build/pallet-coupling)
//! way is great for getting rid of those dependencies for the implementation.
//! Nevertheless, those dependencies still exist in testing because when the
//! `mock.rs` file is defined, you're forced to give some implementations for
//! the associated types of your pallet `Config`.
//!
//! Then, you are mostly forced to use other pallet configurations
//! getting a [tight coupling](https://docs.substrate.io/build/pallet-coupling/)
//! with them. It has some downsides:
//! - You need to learn how to configure other pallets.
//! - You need to know how those pallets work, because they affect directly the
//!   behavior of the pallet you're testing.
//! - The way they work can give you non-completed tests. It means that some
//!   paths of your pallet can not be tested because some dependency works in a
//!   specific way.
//! - You need a lot of effort maintaining your tests because each time one
//!   dependency changes, it can easily break your tests.
//!
//! This doesn't scale well. Frequently some pallet dependencies need in turn to
//! configure their own dependent pallets, making this problem even worse.
//!
//! This is why mocking is so important. It lets you get rid of all these
//! dependencies and related issues, obtaining **loose coupling tests**.
//!
//! There are other crates focusing on this problem,
//! such as [`mockall`](https://docs.rs/mockall/latest/mockall/),
//! but they mock traits. Instead, this crate gives you an entire pallet
//! ready to use in any runtime, implementing the number of traits you specify.
//!
//! ## *Mock pallet* usage
//!
//! Suppose that in our pallet, which we'll call it `my_pallet`, we have an
//! associated type in our `Config`, which implements traits `TraitA` and
//! `TraitB`. Those traits are defined as follows:
//!
//! ```no_run
//! trait TraitA {
//!     type AssocA;
//!
//!     fn foo() -> Self::AssocA;
//! }
//!
//! trait TraitB {
//!     type AssocB;
//!
//!     fn bar(a: u64, b: Self::AssocB) -> u32;
//! }
//! ```
//!
//! We have a really huge pallet that implements a specific behavior for those
//! traits, but we want to get rid of such dependency so we [generate a *mock
//! pallet*](#mock-pallet-creation), we'll call it `pallet_mock_dep`.
//!
//! We can add this *mock pallet* to the runtime as usual:
//!
//! ```ignore
//! frame_support::construct_runtime!(
//!     pub enum Runtime where
//!         Block = Block,
//!         NodeBlock = Block,
//!         UncheckedExtrinsic = UncheckedExtrinsic,
//!     {
//!         System: frame_system,
//!         MockDep: pallet_mock_dep,
//!         MyPallet: my_pallet,
//!     }
//! );
//! ```
//!
//! And we configure it as a regular pallet:
//!
//! ```ignore
//! impl pallet_mock_dep::Config for Runtime {
//!     type AssocA = bool;
//!     type AssocB = u8;
//! }
//! ```
//!
//! Later in our use case, we can give a behavior for both `foo()` and `bar()`
//! methods in their analogous methods `mock_foo()` and `mock_bar()` which
//! accept a closure.
//!
//! ```ignore
//! #[test]
//! fn correct() {
//!     new_test_ext().execute_with(|| {
//!         MockDep::mock_foo(|| true);
//!         MockDep::mock_bar(|a, b| {
//!             assert_eq!(a, 42);
//!             assert_eq!(b, false);
//!             23
//!         });
//!
//!         // This method will call foo() and bar() under the hood, running the
//!         // closures we just have defined.
//!         MyPallet::my_call();
//!     });
//! }
//! ```
//!
//! Take a look to the [pallet tests](`tests/pallet.rs`) to have a user view of
//! how to use a *mock pallet*. It supports any kind of trait, with reference
//! parameters and generics at trait level and method level.
//!
//! ## Mock pallet creation
//!
//! **NOTE: There is a working progress on this part to generate *mock pallets*
//! automatically using procedural macros. Once done, all this part can be
//! auto-generated.**
//!
//! This crate exports two macros [`register_call!()`] and [`execute_call!()`]
//! that allow you to build a *mock pallet*.
//!
//! - [`register_call!()`] registers a closure where you can define the
//! mock behavior for that method. The method which registers the closure must
//! have the name of the trait method you want to mock prefixed with `mock_`.
//!
//! - [`execute_call!()`] is placed in the trait method implementation and will
//!   call the closure previously registered by [`register_call!()`]
//!
//! The only condition to use these macros is to have the following storage in
//! the pallet (it's safe to just copy and paste this snippet in your pallet):
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
//! Following the above example, generating a *mock pallet* for both `TraitA`
//! and `TraitB` is done as follows:
//!
//! ```no_run
//! #[frame_support::pallet]
//! pub mod pallet {
//!     # trait TraitA {
//!     #     type AssocA;
//!     #
//!     #     fn foo() -> Self::AssocA;
//!     # }
//!     #
//!     # trait TraitB {
//!     #     type AssocB;
//!     #
//!     #     fn bar(a: u64, b: Self::AssocB) -> u32;
//!     # }
//!
//!     use frame_support::pallet_prelude::*;
//!     use mock_builder::{execute_call, register_call};
//!
//!     #[pallet::config]
//!     pub trait Config: frame_system::Config {
//!         type AssocA;
//!         type AssocB;
//!     }
//!
//!     #[pallet::pallet]
//!     #[pallet::generate_store(pub(super) trait Store)]
//!     pub struct Pallet<T>(_);
//!
//!     #[pallet::storage]
//!     pub(super) type CallIds<T: Config> = StorageMap<
//!         _,
//!         Blake2_128Concat,
//!         <Blake2_128 as frame_support::StorageHasher>::Output,
//!         mock_builder::CallId,
//!     >;
//!
//!     impl<T: Config> Pallet<T> {
//!         fn mock_foo(f: impl Fn() -> T::AssocA + 'static) {
//!             register_call!(move |()| f())
//!         }
//!
//!         fn mock_bar(f: impl Fn(u64, T::AssocB) -> u32 + 'static) {
//!             register_call!(move |(a, b)| f(a, b))
//!         }
//!     }
//!
//!     impl<T: Config> TraitA for Pallet<T> {
//!         type AssocA = T::AssocA;
//!
//!         fn foo() -> Self::AssocA {
//!             execute_call!(())
//!         }
//!     }
//!
//!     impl<T: Config> TraitB for Pallet<T> {
//!         type AssocB = T::AssocB;
//!
//!         fn bar(a: u64, b: Self::AssocB) -> u32 {
//!             execute_call!((a, b))
//!         }
//!     }
//! }
//! ```
//!
//! If types for the closure of `mock_*` method and trait method don't match,
//! you will obtain a runtime error in your tests.
//!
//! ## Mock Patterns
//!
//! ### Storage pattern
//! In some cases it's pretty common making a mock that returns a value that was
//! set previously by another mock. For this case you can define your "getter"
//! mock inside the definition of the "setter" mock, as follows:
//!
//! ```ignore
//! MyMock::mock_set(|value| MyMock::mock_get(move || value));
//! ```
//!
//! Any call to `get()` will return the last value given to `set()`.
//!
//! ### Check internal calls are ordered
//! If you want to test some mocks method are calle in some order, you can
//! define them nested, in the expected order they must be called
//!
//! ```ignore
//! MyMock::mock_first(|| {
//!     MyMock::mock_second(|| {
//!         MyMock::mock_third(|| {
//!             //...
//!         })
//!     })
//! });
//!
//!
//! // The next method only will be succesful
//! // if it makes the internal calls in order
//! MyPallet::calls_first_second_third();
//! ```

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
/// Same as `register()` but it uses as locator who calls this macro.
#[macro_export]
macro_rules! register_call {
	($f:expr) => {{
		$crate::register::<CallIds<T>, _, _, _, _>(|| (), $f)
	}};
}

/// Execute a function from the function storage.
/// Same as `execute()` but it uses as locator who calls this macro.
#[macro_export]
macro_rules! execute_call {
	($input:expr) => {{
		$crate::execute::<CallIds<T>, _, _, _>(|| (), $input)
	}};
}
