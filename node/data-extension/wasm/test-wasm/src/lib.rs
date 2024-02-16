use std::slice::from_raw_parts;

use serde::{Deserialize, Serialize};
use wasm_utils::pack_ptr_and_len;

#[derive(Debug, Serialize, Deserialize)]
pub struct TestStruct {
	vec: Vec<u8>,
}

#[no_mangle]
pub unsafe fn test_fn(input_ptr: *mut u8, input_len: usize) -> u64 {
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

// #[cfg(not(feature = "std"))]
//     #[no_mangle]
//     pub unsafe fn OffchainWorkerApi_offchain_worker(input_data: *mut u8,
// input_len: usize) -> u64 {         let mut input = if input_len == 0 { &[0u8;
// 0] } else { unsafe { sp_api::slice::from_raw_parts(input_data, input_len) }
// };         sp_api::init_runtime_logger();
//         let output = (move || {
//             let header: <Block as BlockT>::Header = match
// sp_api::DecodeLimit::decode_all_with_depth_limit(sp_api::MAX_EXTRINSIC_DEPTH,
// &mut input) {                 Ok(res) => res,
//                 Err(e) => panic!("Bad input data provided to {}: {}",
// "offchain_worker", e),             };
//             #[allow(deprecated)] <Runtime as
// sp_offchain::runtime_decl_for_offchain_worker_api::OffchainWorkerApi<Block>>::offchain_worker(&
// header)         })();
//         sp_api::to_substrate_wasm_fn_return_value(&output)
//     }
