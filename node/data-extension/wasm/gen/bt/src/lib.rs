use std::slice::from_raw_parts;

use serde::{Deserialize, Serialize};
use wasm_utils::pack_ptr_and_len;

#[derive(Debug, Serialize, Deserialize)]
pub struct TestStruct {
	vec: Vec<u8>,
}

#[no_mangle]
pub unsafe fn test_slice(input_ptr: *mut u8, input_len: usize) -> u64 {
	let ret = if input_len == 0 {
		&[0u8; 0]
	} else {
		unsafe { from_raw_parts(input_ptr, input_len) }
	};

	let mut r: TestStruct = serde_json::from_slice(ret).expect("can decode TestStruct");

	r.vec.push(11);

	let res = serde_json::to_vec(&r).expect("can encode test struct");

	pack_ptr_and_len(res.as_slice())
}
