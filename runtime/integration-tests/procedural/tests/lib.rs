// NOTE: to show the output during the compilation ensure this file is modified
// before compiling and the feature is enabled:
// Steps:
// 1. touch some file from this crate
// 2. `cargo test -p runtime-integration-tests-proc-macro -F debug-proc-macros`

#![allow(unused)]
#![cfg(feature = "debug-proc-macros")]

#[macro_use]
extern crate runtime_integration_tests_proc_macro;

#[__dbg_test_runtimes(all)]
fn macro_runtimes() {}

#[__dbg_test_runtimes([development, altair, centrifuge])]
fn macro_runtimes_list() {}

#[__dbg_test_runtimes(all, ignore = "reason")]
fn macro_runtimes_ignored() {}

#[__dbg_test_runtimes([development, altair, centrifuge], ignore = "reason")]
fn macro_runtimes_list_ignored() {}
