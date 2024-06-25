pub mod env;
pub mod envs {
	pub mod evm_env;
	pub mod fudge_env;
	pub mod runtime_env;
}
pub mod config;
mod impls;
pub mod utils;

// Test cases
mod cases {
	mod account_derivation;
	mod assets;
	mod block_rewards;
	mod currency_conversions;
	mod ethereum_transaction;
	mod example;
	mod investments;
	mod liquidity_pools;
	mod loans;
	mod lp;
	mod oracles;
	mod precompile;
	mod proxy;
	mod restricted_transfers;
	mod xcm_transfers;
}

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
