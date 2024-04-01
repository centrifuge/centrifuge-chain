// For generic modules we can reactive the unused warn disabled on lib.rs
#![warn(unused)]
// Allow dead code for utilities not used yet
#![allow(dead_code)]

pub mod env;
pub mod envs {
	pub mod fudge_env;
	pub mod runtime_env;
}
pub mod config;
mod impls;
pub mod utils;

// Test cases
mod cases {
	mod account_derivation;
	mod example;
	mod investments;
	mod liquidity_pools;
	mod loans;
	mod oracles;
	mod proxy;
	mod restricted_transfers;
	mod rewards;
}

/// Generate tests for the specified runtimes or all runtimes.
/// Usage
///
/// ```rust
/// use crate::generic::config::Runtime;
///
/// fn foo<T: Runtime> {
///     /// Your test here...
/// }
///
/// crate::test_for_runtimes!([development, altair, centrifuge], foo);
/// ```
/// For the following command: `cargo test -p runtime-integration-tests foo`,
/// it will generate the following output:
///
/// ```text
/// test generic::foo::altair ... ok
/// test generic::foo::development ... ok
/// test generic::foo::centrifuge ... ok
/// ```
///
/// Available input  for the first argument is:
/// - Any combination of `development`, `altair`, `centrifuge` inside `[]`.
/// - The world `all`.
#[macro_export]
macro_rules! test_for_runtimes {
	( [ $($runtime_name:ident),* ], $test_name:ident ) => {
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
		$crate::test_for_runtimes!([development, altair, centrifuge], $test_name);
    };
}
