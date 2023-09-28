#![cfg(test)]

use cfg_primitives::{Balance, PoolId, TrancheId};
use cfg_types::{
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use sp_runtime::AccountId32;

// Add your tests for your use case here:
mod use_case {
	mod loans;
}
pub mod util;

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
	const KIND: RuntimeKind;
}

#[cfg(feature = "compile-runtimes")]
mod __impl {
	use super::*;

	impl Config for development_runtime::Runtime {
		const KIND: RuntimeKind = RuntimeKind::Development;
	}
	impl Config for altair_runtime::Runtime {
		const KIND: RuntimeKind = RuntimeKind::Altair;
	}
	impl Config for centrifuge_runtime::Runtime {
		const KIND: RuntimeKind = RuntimeKind::Centrifuge;
	}
}

pub enum RuntimeKind {
	Development,
	Altair,
	Centrifuge,
}

#[macro_export]
macro_rules! test_with_all_runtimes {
	($setup:ident, $name:ident) => {
		mod $name {
			use super::*;

			#[test]
			#[cfg(feature = "compile-runtimes")]
			fn development() {
				$setup::<development_runtime::Runtime>()
					.execute_with($name::<development_runtime::Runtime>);
			}

			#[test]
			#[cfg(feature = "compile-runtimes")]
			fn altair() {
				$setup::<altair_runtime::Runtime>().execute_with($name::<altair_runtime::Runtime>);
			}

			#[test]
			#[cfg(feature = "compile-runtimes")]
			fn centrifuge() {
				$setup::<centrifuge_runtime::Runtime>()
					.execute_with($name::<centrifuge_runtime::Runtime>);
			}

			#[test]
			#[cfg(not(feature = "compile-runtimes"))]
			fn no_runtime() {
				panic!("Compile without '--no-default-features' to run the tests with runtimes");
			}
		}
	};
}
