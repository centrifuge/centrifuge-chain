use std::{any::Any, cell::RefCell, collections::HashMap, fmt};

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

#[derive(Debug, PartialEq)]
pub enum Error {
	CallNotFound,
	TypeNotMatch,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Error::CallNotFound => "Trying to call a function that is not registered",
				Error::TypeNotMatch => "The function is registered but the type mismatches",
			}
		)
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
pub fn execute_call<Args: 'static, R: 'static>(call_id: CallId, args: Args) -> Result<R, Error> {
	CALLS.with(|state| {
		let registry = &*state.borrow();
		let call = registry.get(&call_id).ok_or(Error::CallNotFound)?;
		call.as_any()
			.downcast_ref::<CallWrapper<Args, R>>()
			.map(|wrapper| wrapper.0(args))
			.ok_or(Error::TypeNotMatch)
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn correct_type() {
		let func_1 = |n: u8| -> usize { 23 * n as usize };
		let call_id_1 = register_call(func_1);
		let result = execute_call::<_, usize>(call_id_1, 2u8);

		assert_eq!(result, Ok(46));
	}

	#[test]
	fn different_input_type() {
		let func_1 = |n: u8| -> usize { 23 * n as usize };
		let call_id_1 = register_call(func_1);
		let result = execute_call::<_, usize>(call_id_1, 'a');

		assert_eq!(result, Err(Error::TypeNotMatch));
	}

	#[test]
	fn different_output_type() {
		let func_1 = |n: u8| -> usize { 23 * n as usize };
		let call_id_1 = register_call(func_1);
		let result = execute_call::<_, char>(call_id_1, 2u8);

		assert_eq!(result, Err(Error::TypeNotMatch));
	}

	#[test]
	fn no_registered() {
		let call_id_1 = 42;

		assert_eq!(
			execute_call::<_, usize>(call_id_1, 2u8),
			Err(Error::CallNotFound)
		);
	}
}
