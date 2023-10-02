use cfg_primitives::{Balance, BlockNumber, CollectionId, ItemId, Moment, PoolId, TrancheId};
use cfg_types::{
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use fudge::primitives::Chain;
use polkadot_core_primitives::Block as RelayBlock;
use polkadot_primitives::runtime_api::ParachainHost;
use sp_api::ApiExt;
use sp_block_builder::BlockBuilder;
use sp_runtime::AccountId32;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;

pub mod env;
pub mod util;

#[macro_export]
macro_rules! test_with_all_runtimes {
	($setup:ident, $name:ident) => {
		mod $name {
			use super::*;

			#[test]
			fn development() {
				$setup::<development_runtime::Runtime>()
					.execute_with($name::<development_runtime::Runtime>);
			}

			#[test]
			fn altair() {
				$setup::<altair_runtime::Runtime>().execute_with($name::<altair_runtime::Runtime>);
			}

			#[test]
			fn centrifuge() {
				$setup::<centrifuge_runtime::Runtime>()
					.execute_with($name::<centrifuge_runtime::Runtime>);
			}
		}
	};
}
