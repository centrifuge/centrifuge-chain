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
//! [`register_call!()`] and [`execute_call!()`] expect the following storage in the pallet.
//! It's safe to just copy and paste in your pallet mock.
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
//!	    _,
//!	    Blake2_128Concat,
//!	    <Blake2_128 as frame_support::StorageHasher>::Output,
//!	    mock_builder::CallId,
//! >;
//!
//! # }
//! ```
//!
//! Take a look to the [pallet tests](`tests/pallet.rs`) to have a user view of how to use this crate.

/// Provide methods for register/execute calls
pub mod storage {
	use std::{any::Any, cell::RefCell, collections::HashMap};

	/// Identify a call in the call storage
	pub type CallId = u64;

	trait Callable {
		fn as_any(&self) -> &dyn Any;
	}

	thread_local! {
		static CALLS: RefCell<HashMap<CallId, Box<dyn Callable>>>
			= RefCell::new(HashMap::default());
	}

	struct CallWrapper<Input, Output>(Box<dyn Fn(Input) -> Output>);

	impl<Input: 'static, Output: 'static> Callable for CallWrapper<Input, Output> {
		fn as_any(&self) -> &dyn Any {
			self
		}
	}

	/// Register a call into the call storage.
	/// The registered call can be uniquely identified by the returned `CallId`.
	pub fn register_call<F: Fn(Args) -> R + 'static, Args: 'static, R: 'static>(f: F) -> CallId {
		CALLS.with(|state| {
			let registry = &mut *state.borrow_mut();
			let call_id = registry.len() as u64;
			registry.insert(call_id, Box::new(CallWrapper(Box::new(f))));
			call_id
		})
	}

	/// Execute a call from the call storage identified by a `call_id`.
	pub fn execute_call<Args: 'static, R: 'static>(call_id: CallId, args: Args) -> R {
		CALLS.with(|state| {
			let registry = &*state.borrow();
			let call = registry.get(&call_id).unwrap();
			call.as_any()
				.downcast_ref::<CallWrapper<Args, R>>()
				.expect("Bad mock implementation: expected other function type")
				.0(args)
		})
	}
}

pub use storage::CallId;

/// Prefix that the register functions should have.
pub const MOCK_FN_PREFIX: &str = "mock_";

/// Gives the absolute string identification of a function.
#[macro_export]
macro_rules! function_locator {
	() => {{
		// Aux function to extract the path
		fn f() {}

		fn type_name_of<T>(_: T) -> &'static str {
			std::any::type_name::<T>()
		}
		let name = type_name_of(f);
		&name[..name.len() - "::f".len()]
	}};
}

/// Gives the string identification of a function.
/// The identification will be the same no matter if it belongs to a trait or has an `except_`
/// prefix name.
#[macro_export]
macro_rules! call_locator {
	() => {{
		let path_name = $crate::function_locator!();
		let (path, name) = path_name.rsplit_once("::").expect("always ::");

		let base_name = name.strip_prefix($crate::MOCK_FN_PREFIX).unwrap_or(name);
		let correct_path = path
			.strip_prefix("<")
			.map(|trait_path| trait_path.split_once(" as").expect("always ' as'").0)
			.unwrap_or(path);

		format!("{}::{}", correct_path, base_name)
	}};
}

/// Register a call into the call storage.
/// This macro should be called from the method that wants to register `f`.
/// This macro must be called from a pallet with the `CallIds` storage.
/// Check the main documentation.
#[macro_export]
macro_rules! register_call {
	($f:expr) => {{
		use frame_support::StorageHasher;

		CallIds::<T>::insert(
			frame_support::Blake2_128::hash($crate::call_locator!().as_bytes()),
			$crate::storage::register_call($f),
		);
	}};
}

/// Execute a call from the call storage.
/// This macro should be called from the method that wants to execute `f`.
/// This macro must be called from a pallet with the `CallIds` storage.
/// Check the main documentation.
#[macro_export]
macro_rules! execute_call {
	($params:expr) => {{
		use frame_support::StorageHasher;

		let hash = frame_support::Blake2_128::hash($crate::call_locator!().as_bytes());
		$crate::storage::execute_call(
			CallIds::<T>::get(hash).expect(&format!(
				"Called to {}, but mock was not found",
				$crate::call_locator!()
			)),
			$params,
		)
	}};
}

#[cfg(test)]
mod tests {
	trait TraitExample {
		fn function_locator() -> String;
		fn call_locator() -> String;
	}

	struct Example;

	impl Example {
		fn mock_function_locator() -> String {
			function_locator!().into()
		}

		fn mock_call_locator() -> String {
			call_locator!().into()
		}
	}

	impl TraitExample for Example {
		fn function_locator() -> String {
			function_locator!().into()
		}

		fn call_locator() -> String {
			call_locator!().into()
		}
	}

	#[test]
	fn function_locator() {
		assert_eq!(
			Example::mock_function_locator(),
			"mock_builder::tests::Example::mock_function_locator"
		);

		assert_eq!(
			Example::function_locator(),
			"<mock_builder::tests::Example as \
            mock_builder::tests::TraitExample>::function_locator"
		);
	}

	#[test]
	fn call_locator() {
		assert_eq!(
			Example::call_locator(),
			"mock_builder::tests::Example::call_locator"
		);

		assert_eq!(Example::call_locator(), Example::mock_call_locator());
	}
}
