use std::{cell::RefCell, collections::HashMap, fmt};

/// Identify a call in the call storage
pub type CallId = u64;

#[derive(Debug, PartialEq, Clone)]
pub struct TypeSignature(String);

impl TypeSignature {
	pub fn new<I, O>() -> TypeSignature {
		Self(format!(
			"{}->{}",
			std::any::type_name::<I>(),
			std::any::type_name::<O>(),
		))
	}
}

struct CallInfo {
	ptr: u128,
	type_signature: TypeSignature,
}

thread_local! {
	static CALLS: RefCell<HashMap<CallId, CallInfo>> = RefCell::new(HashMap::default());
}

#[derive(Debug, PartialEq)]
pub enum Error {
	CallNotFound,
	TypeNotMatch(TypeSignature, TypeSignature),
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::CallNotFound => write!(f, "Trying to call a function that is not registered"),
			Error::TypeNotMatch(expected, found) => write!(
				f,
				"The function is registered but the type mismatches. Expected {}, found: {}",
				expected.0, found.0
			),
		}
	}
}

/// Register a call into the call storage.
/// The registered call can be uniquely identified by the returned `CallId`.
pub fn register_call<F: Fn(Args) -> R + 'static, Args, R>(f: F) -> CallId {
	let f = Box::new(f) as Box<dyn Fn(Args) -> R>;
	let p = Box::into_raw(f);

	// SAFETY: transforming a wide pointer to an u128 is always safe.
	let call = CallInfo {
		ptr: unsafe { std::mem::transmute(p) },
		type_signature: TypeSignature::new::<Args, R>(),
	};

	CALLS.with(|state| {
		let registry = &mut *state.borrow_mut();
		let call_id = registry.len() as u64;
		registry.insert(call_id, call);
		call_id
	})
}

/// Execute a call from the call storage identified by a `call_id`.
pub fn execute_call<Args, R>(call_id: CallId, args: Args) -> Result<R, Error> {
	let expected_type_signature = TypeSignature::new::<Args, R>();

	CALLS.with(|state| {
		let registry = &*state.borrow();
		let call = registry.get(&call_id).ok_or(Error::CallNotFound)?;

		if expected_type_signature != call.type_signature {
			return Err(Error::TypeNotMatch(
				expected_type_signature,
				call.type_signature.clone(),
			));
		}

		// SAFETY: The existance of this clousure is ensured by the forget call below.
		// The type of the transmuted call is ensured in runtime by the above type signature
		// check.
		let f = unsafe {
			let p: *mut dyn Fn(Args) -> R = std::mem::transmute(call.ptr);
			Box::from_raw(p)
		};

		let output = f(args);

		std::mem::forget(f);
		Ok(output)
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

		assert_eq!(
			result,
			Err(Error::TypeNotMatch(
				TypeSignature::new::<char, usize>(),
				TypeSignature::new::<u8, usize>()
			))
		);
	}

	#[test]
	fn different_output_type() {
		let func_1 = |n: u8| -> usize { 23 * n as usize };
		let call_id_1 = register_call(func_1);
		let result = execute_call::<_, char>(call_id_1, 2u8);

		assert_eq!(
			result,
			Err(Error::TypeNotMatch(
				TypeSignature::new::<u8, char>(),
				TypeSignature::new::<u8, usize>()
			))
		);
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
