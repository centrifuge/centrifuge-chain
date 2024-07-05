// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// Allow dead code for utilities not used yet
#![allow(dead_code)]
// All code in this crate is test related
#![cfg(test)]

// Allow `#[test_runtimes]` macro to be called everywhere in the crate
#[macro_use]
extern crate runtime_integration_tests_proc_macro;

mod cases;
mod env;
mod utils;
mod envs {
	pub mod evm_env;
	pub mod fudge_env;
	pub mod runtime_env;
}
mod config;
mod impls;

/// Generate tests for the specified runtimes or all runtimes.
/// Usage. Used as building block for #[test_runtimes] procedural macro.
///
/// NOTE: Do not use it direclty, use `#[test_runtimes]` proc macro instead
#[macro_export]
macro_rules! __test_for_runtimes {
	( [ $($runtime_name:ident),* ], $test_name:ident ) => {
        #[cfg(test)]
		mod $test_name {
			use super::*;

            #[allow(unused)]
            use development_runtime as development;

            #[allow(unused)]
            use altair_runtime as altair;

            #[allow(unused)]
            use centrifuge_runtime as centrifuge;

            $(
                #[tokio::test]
                async fn $runtime_name() {
                    $test_name::<$runtime_name::Runtime>()
                }
            )*
		}
	};
	( all , $test_name:ident ) => {
		$crate::__test_for_runtimes!([development, altair, centrifuge], $test_name);
    };
}
