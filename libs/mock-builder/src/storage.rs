//! This module is in change of storing closures with the type `Fn(I) -> O`
//! in a static lifetime storage, supporting mixing differents `I` and `O`
//! types. Because we need to merge different closures with different types in
//! the same storage, we use an `u128` as closure identification (composed by
//! the closure function pointer (`u64`) and the pointer to the closure metadata
//! (`u64`).

use std::{
	cell::RefCell,
	collections::HashMap,
	fmt,
	sync::{Arc, Mutex},
};

use super::util::TypeSignature;

/// Identify a call in the call storage
pub type CallId = u64;

struct CallInfo {
	/// Closure identification
	ptr: u128,

	/// Runtime representation of the closure type.
	/// This field is needed to ensure we are getting the correct closure type,
	/// since the type at compiler time is lost in the `u128` representation of
	/// the closure.
	type_signature: TypeSignature,
}

type Registry = HashMap<CallId, Arc<Mutex<CallInfo>>>;

thread_local! {
	static CALLS: RefCell<Registry> = RefCell::new(HashMap::default());
}

#[derive(Debug, PartialEq)]
pub enum Error {
	CallNotFound,
	TypeNotMatch {
		expected: TypeSignature,
		found: TypeSignature,
	},
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::CallNotFound => write!(f, "Trying to call a function that is not registered"),
			Error::TypeNotMatch { expected, found } => write!(
				f,
				"The function is registered but the type mismatches. Expected {expected}, found: {found}",
			),
		}
	}
}

/// Register a call into the call storage.
/// The registered call can be uniquely identified by the returned `CallId`.
pub fn register_call<F: Fn(I) -> O + 'static, I, O>(f: F) -> CallId {
	// We box the closure in order to store it in a fixed place of memory,
	// and handle it in a more generic way without knowing the specific closure
	// implementation.
	let f = Box::new(f) as Box<dyn Fn(I) -> O>;

	// We're only interested in the memory address of the closure.
	// Box is never dropped after this call.
	let ptr: *const dyn Fn(I) -> O = Box::into_raw(f);

	let call = CallInfo {
		// We need the transmutation to forget about the type of the closure at compile time,
		// and then store closures with different types together.
		// SAFETY: transforming a wide pointer (*const dyn) to an u128 is always safe
		// because the memory representation is the same.
		ptr: unsafe { std::mem::transmute(ptr) },
		// Since we've lost the type representation at compile time, we need to store the type
		// representation at runtime, in order to recover later the correct closure
		type_signature: TypeSignature::new::<I, O>(),
	};

	CALLS.with(|state| {
		let registry = &mut *state.borrow_mut();
		let call_id = registry.len() as u64;
		registry.insert(call_id, Arc::new(Mutex::new(call)));
		call_id
	})
}

/// Execute a call from the call storage identified by a `call_id`.
pub fn execute_call<I, O>(call_id: CallId, input: I) -> Result<O, Error> {
	let expected_type_signature = TypeSignature::new::<I, O>();

	let call = CALLS.with(|state| {
		let registry = &*state.borrow();
		let call = registry.get(&call_id).ok_or(Error::CallNotFound)?;
		Ok(call.clone())
	})?;

	let call = call.lock().unwrap();

	// We need the runtime type check since we lost the type at compile time.
	if expected_type_signature != call.type_signature {
		return Err(Error::TypeNotMatch {
			expected: expected_type_signature,
			found: call.type_signature.clone(),
		});
	}

	// SAFETY:
	// 1. The existence of this closure ptr in consequent calls is ensured
	// thanks to Box::into_raw() at register_call(),
	// which takes the Box ownership without dropping it. So, ptr exists forever.
	// 2. The type of the transmuted call is ensured in runtime by the above type
	// signature check.
	// 3. The pointer is correctly aligned because it was allocated by a Box.
	// 4. The closure is called once at the same time thanks to the mutex.
	let f: &dyn Fn(I) -> O = unsafe {
		#[allow(clippy::useless_transmute)] // Clippy hints something erroneous
		let ptr: *const dyn Fn(I) -> O = std::mem::transmute(call.ptr);
		&*ptr
	};

	Ok(f(input))
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
			Err(Error::TypeNotMatch {
				expected: TypeSignature::new::<char, usize>(),
				found: TypeSignature::new::<u8, usize>()
			})
		);
	}

	#[test]
	fn different_output_type() {
		let func_1 = |n: u8| -> usize { 23 * n as usize };
		let call_id_1 = register_call(func_1);
		let result = execute_call::<_, char>(call_id_1, 2u8);

		assert_eq!(
			result,
			Err(Error::TypeNotMatch {
				expected: TypeSignature::new::<u8, char>(),
				found: TypeSignature::new::<u8, usize>()
			})
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
