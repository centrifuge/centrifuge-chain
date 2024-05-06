#![allow(unused)]
#![cfg(feature = "debug-proc-macros")]

#[macro_use]
extern crate runtime_integration_tests_proc_macro;

#[__dbg_test_runtimes(all)]
fn macro_runtimes() {}

#[__dbg_test_runtimes([development, altair, centrifuge])]
fn macro_runtimes_list() {}
