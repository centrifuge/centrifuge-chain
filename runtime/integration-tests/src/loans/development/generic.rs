use cfg_primitives::{Balance, PoolId};
use cfg_types::{
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata},
};
use frame_support::{
	assert_noop,
	traits::{GenesisBuild, Get, OriginTrait},
	BoundedVec,
};
use pallet_pool_system::tranches::{TrancheInput, TrancheLoc, TrancheType};
use sp_runtime::{traits::One, AccountId32, Perquintill};

trait_set::trait_set! {
	pub trait Config =
		frame_system::Config<AccountId = AccountId32>
		+ pallet_pool_system::Config<CurrencyId = CurrencyId, Balance = Balance, PoolId = PoolId>
		+ pallet_balances::Config<Balance = Balance>
		+ pallet_investments::Config
		+ pallet_pool_registry::Config<CurrencyId = CurrencyId, PoolId = PoolId, Balance = Balance>
		+ pallet_permissions::Config<Role = Role, Scope = PermissionScope<PoolId, CurrencyId>>
		+ orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>
		+ orml_asset_registry::Config<
			AssetId = CurrencyId,
			CustomMetadata = CustomMetadata,
			Balance = Balance,
		>;
}

#[macro_export]
macro_rules! test_for_all_runtimes {
	($setup:ident, $name:ident) => {
		mod $name {
			use super::*;

			#[test]
			fn development() {
				$setup::<development_runtime::Runtime>()
					.execute_with($name::<development_runtime::Runtime>);
			}

			fn altair() {
				$setup::<altair_runtime::Runtime>().execute_with($name::<altair_runtime::Runtime>);
			}

			fn centrifuge() {
				$setup::<centrifuge_runtime::Runtime>()
					.execute_with($name::<centrifuge_runtime::Runtime>);
			}
		}
	};
}
