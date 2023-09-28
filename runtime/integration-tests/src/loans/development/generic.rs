use cfg_primitives::{Balance, PoolId, TrancheId};
use cfg_types::{
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use frame_support::{
	assert_noop,
	traits::{GenesisBuild, Get, OriginTrait},
	BoundedVec,
};
use pallet_pool_system::tranches::{TrancheInput, TrancheLoc, TrancheType};
use sp_runtime::{traits::One, AccountId32, Perquintill};

pub enum RuntimeKind {
	Development,
	Altair,
	Centrifuge,
}

pub trait Config:
	frame_system::Config<AccountId = AccountId32>
	+ pallet_pool_system::Config<
		CurrencyId = CurrencyId,
		Balance = Balance,
		PoolId = PoolId,
		TrancheId = TrancheId,
	> + pallet_balances::Config<Balance = Balance>
	+ pallet_investments::Config<InvestmentId = TrancheCurrency, Amount = Balance>
	+ pallet_pool_registry::Config<
		CurrencyId = CurrencyId,
		PoolId = PoolId,
		Balance = Balance,
		ModifyPool = pallet_pool_system::Pallet<Self>,
		ModifyWriteOffPolicy = pallet_loans::Pallet<Self>,
	> + pallet_permissions::Config<Role = Role, Scope = PermissionScope<PoolId, CurrencyId>>
	+ pallet_loans::Config<Balance = Balance, PoolId = PoolId>
	+ orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>
	+ orml_asset_registry::Config<
		AssetId = CurrencyId,
		CustomMetadata = CustomMetadata,
		Balance = Balance,
	>
{
	const RuntimeKind: RuntimeKind;
}

impl Config for development_runtime::Runtime {
	const RuntimeKind: RuntimeKind = RuntimeKind::Development;
}
impl Config for altair_runtime::Runtime {
	const RuntimeKind: RuntimeKind = RuntimeKind::Altair;
}
impl Config for centrifuge_runtime::Runtime {
	const RuntimeKind: RuntimeKind = RuntimeKind::Centrifuge;
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
