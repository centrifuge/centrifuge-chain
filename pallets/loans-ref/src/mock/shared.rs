pub mod storage {
	use std::{any::Any, cell::RefCell, collections::HashMap};

	trait Callable {
		fn as_any(&self) -> &dyn Any;
	}

	impl<Input: 'static, Output: 'static> Callable for FnWrapper<Input, Output> {
		fn as_any(&self) -> &dyn Any {
			self
		}
	}

	thread_local! {
		static CALLS: RefCell<HashMap<CallId, Box<dyn Callable>>>
			= RefCell::new(HashMap::default());
	}

	struct FnWrapper<Input, Output>(Box<dyn Fn(Input) -> Output>);

	pub type CallId = u64;

	pub fn register_call<F: Fn(Args) -> R + 'static, Args: 'static, R: 'static>(f: F) -> CallId {
		CALLS.with(|state| {
			let registry = &mut *state.borrow_mut();
			let call_id = registry.len() as u64;
			registry.insert(call_id, Box::new(FnWrapper(Box::new(f))));
			call_id
		})
	}

	pub fn execute_call<Args: 'static, R: 'static>(call_id: CallId, args: Args) -> R {
		CALLS.with(|state| {
			let registry = &*state.borrow();
			let call = registry.get(&call_id).unwrap();
			call.as_any()
				.downcast_ref::<FnWrapper<Args, R>>()
				.expect("Expected other function type")
				.0(args)
		})
	}
}

pub use storage::CallId;

pub const EXPECTATION_FN_PREFIX: &str = "expect_";
pub const EXPECT_CALL_MSG: &str = "Must be an expectation for this call";

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

#[macro_export]
macro_rules! call_locator {
	() => {{
		let path_name = crate::function_locator!();
		let (path, name) = path_name.rsplit_once("::").expect("always ::");

		let base_name = name
			.strip_prefix(crate::mock::shared::EXPECTATION_FN_PREFIX)
			.unwrap_or(name);
		let correct_path = path
			.strip_prefix("<")
			.map(|trait_path| trait_path.split_once(" as").expect("always ' as'").0)
			.unwrap_or(path);

		format!("{}::{}", correct_path, base_name)
	}};
}

#[macro_export]
macro_rules! register_call {
	($f:expr) => {{
		use frame_support::StorageHasher;

		println!("register >> {}", crate::call_locator!());

		CallIds::<T>::insert(
			frame_support::Blake2_128::hash(crate::call_locator!().as_bytes()),
			crate::mock::shared::storage::register_call($f),
		);
	}};
}

#[macro_export]
macro_rules! execute_call {
	($params:expr) => {{
		use frame_support::StorageHasher;

		println!("execute >> {}", crate::call_locator!());

		let hash = frame_support::Blake2_128::hash(crate::call_locator!().as_bytes());
		crate::mock::shared::storage::execute_call(
			CallIds::<T>::get(hash).expect(crate::mock::shared::EXPECT_CALL_MSG),
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
		fn expect_function_locator() -> String {
			function_locator!().into()
		}

		fn expect_call_locator() -> String {
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
			Example::expect_function_locator(),
			"pallet_loans_ref::mock::shared::tests::Example::expect_function_locator"
		);

		assert_eq!(
			Example::function_locator(),
			"<pallet_loans_ref::mock::shared::tests::Example as \
            pallet_loans_ref::mock::shared::tests::TraitExample>::function_locator"
		);
	}

	#[test]
	fn call_locator() {
		assert_eq!(
			Example::call_locator(),
			"pallet_loans_ref::mock::shared::tests::Example::call_locator"
		);

		assert_eq!(Example::call_locator(), Example::expect_call_locator());
	}
}
