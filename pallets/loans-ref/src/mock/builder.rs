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

	struct FnWrapper<Input, Output>(Box<dyn Fn(Input) -> Output>);

	impl<Input: 'static, Output: 'static> Callable for FnWrapper<Input, Output> {
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
			registry.insert(call_id, Box::new(FnWrapper(Box::new(f))));
			call_id
		})
	}

	/// Execute a call from the call storage identified by a `call_id`.
	pub fn execute_call<Args: 'static, R: 'static>(call_id: CallId, args: Args) -> R {
		CALLS.with(|state| {
			let registry = &*state.borrow();
			let call = registry.get(&call_id).unwrap();
			call.as_any()
				.downcast_ref::<FnWrapper<Args, R>>()
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
		fn f() {}
		fn type_name_of<T>(_: T) -> &'static str {
			std::any::type_name::<T>()
		}
		let name = type_name_of(f);
		&name[..name.len() - 3]
	}};
}

/// Gives the string identification of a function.
/// The identification will be the same no matter if it belongs to a trait or has an `except_`
/// prefix name.
#[macro_export]
macro_rules! call_locator {
	() => {{
		let path_name = crate::function_locator!();
		let (path, name) = path_name.rsplit_once("::").expect("always ::");

		let base_name = name
			.strip_prefix(crate::mock::builder::MOCK_FN_PREFIX)
			.unwrap_or(name);

		let correct_path = path
			.strip_prefix("<")
			.map(|trait_path| trait_path.split_once(" as").expect("always ' as'").0)
			.unwrap_or(path);

		format!("{}::{}", correct_path, base_name)
	}};
}

/// Register a call into the call storage.
/// This macro should be called from the method that wants to register `f`.
/// This macro must be called from a pallet with the following storage:
/// ```no_run
/// #[pallet::storage]
/// pub(super) type CallIds<T: Config> = StorageMap<
/// 	_,
/// 	Blake2_128Concat,
/// 	<Blake2_128 as frame_support::StorageHasher>::Output,
/// 	CallId,
/// >;
/// ```
#[macro_export]
macro_rules! register_call {
	($f:expr) => {{
		use frame_support::StorageHasher;

		CallIds::<T>::insert(
			frame_support::Blake2_128::hash(crate::call_locator!().as_bytes()),
			crate::mock::builder::storage::register_call($f),
		);
	}};
}

/// Execute a call from the call storage.
/// This macro should be called from the method that wants to execute `f`.
/// This macro must be called from a pallet with the following storage:
/// ```no_run
/// #[pallet::storage]
/// pub(super) type CallIds<T: Config> = StorageMap<
/// 	_,
/// 	Blake2_128Concat,
/// 	<Blake2_128 as frame_support::StorageHasher>::Output,
/// 	CallId,
/// >;
/// ```
#[macro_export]
macro_rules! execute_call {
	($params:expr) => {{
		use frame_support::StorageHasher;

		let hash = frame_support::Blake2_128::hash(crate::call_locator!().as_bytes());
		crate::mock::builder::storage::execute_call(
			CallIds::<T>::get(hash).expect(&format!(
				"Called to {}, but mock was not found",
				crate::call_locator!()
			)),
			$params,
		)
	}};
}

#[cfg(test)]
mod tests {

	struct Example;

	trait TraitExample {
		fn function_locator() -> String;
		fn call_locator() -> String;
	}

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
			"pallet_loans_ref::mock::builder::tests::Example::mock_function_locator"
		);

		assert_eq!(
			Example::function_locator(),
			"<pallet_loans_ref::mock::builder::tests::Example as \
            pallet_loans_ref::mock::builder::tests::TraitExample>::function_locator"
		);
	}

	#[test]
	fn call_locator() {
		assert_eq!(
			Example::call_locator(),
			"pallet_loans_ref::mock::builder::tests::Example::call_locator"
		);

		assert_eq!(Example::call_locator(), Example::mock_call_locator());
	}
}
