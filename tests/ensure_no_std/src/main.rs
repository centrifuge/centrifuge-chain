#![no_std]
#![no_main]
// In order to test for no_std compatability simply add new dependencies into the Cargo.toml
// and use them here like the following:
//
// #[allow(unused_imports)]
// use YOUR_DEPENDENCY_ROOT_HERE
//
// Compiling the create now with:
// cargo rustc --target wasm32-unknown-unknown
//
// should be successful. If not, the new dependency is not no_std compatible.

// Dependencies to test please here
#[allow(unused_imports)]
use getrandom;
#[allow(unused_imports)]
use rand_core;
#[allow(unused_imports)]
use schnorrkel;

// Do not change anything below

use core::panic::PanicInfo;
/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
	loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
	loop {}
}
