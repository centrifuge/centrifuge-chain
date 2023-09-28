use cfg_primitives::{Balance, Moment, PoolId};
use cfg_traits::investments::TrancheCurrency as TrancheCurrencyT;
use cfg_types::{
	permissions::{PermissionScope, PoolRole, Role},
	pools::TrancheMetadata,
	tokens::{AssetMetadata, CurrencyId, CustomMetadata, TrancheCurrency},
};
use frame_support::{
	traits::{GenesisBuild, Get},
	BoundedVec,
};
use frame_system::RawOrigin;
use pallet_pool_system::tranches::{TrancheInput, TrancheLoc, TrancheType};
use sp_core::{sr25519::Pair, Pair as _};
use sp_runtime::{traits::One, AccountId32, Perquintill};

use crate::{Config, RuntimeKind};

pub const MUSD_DECIMALS: u32 = 6;
pub const MUSD_UNIT: Balance = 10u128.pow(MUSD_DECIMALS);
pub const MUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(23);

pub const POOL_FUNDS: Balance = 100_000_000 * MUSD_UNIT;

#[derive(Debug, Clone, Copy)]
enum Account {
	Admin,
	Borrower,
	Investor,
}

impl Account {
	fn id(&self) -> AccountId32 {
		Pair::from_string(&format!("//{:?}", self), None)
			.unwrap()
			.public()
			.0
			.into()
	}
}

pub fn new_ext<T: Config>() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<T>()
		.unwrap();

	pallet_balances::GenesisConfig::<T> {
		balances: vec![
			(
				Account::Admin.id(),
				T::PoolDeposit::get() + T::ExistentialDeposit::get(),
			),
			(Account::Borrower.id(), T::ExistentialDeposit::get()),
			(Account::Investor.id(), T::ExistentialDeposit::get()),
		],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	orml_tokens::GenesisConfig::<T> {
		balances: vec![
			(Account::Admin.id(), MUSD_CURRENCY_ID, 1 * MUSD_UNIT),
			(Account::Borrower.id(), MUSD_CURRENCY_ID, 1 * MUSD_UNIT),
			(Account::Investor.id(), MUSD_CURRENCY_ID, POOL_FUNDS),
		],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	sp_io::TestExternalities::new(storage)
}

pub fn register_usdt<T: Config>() {
	orml_asset_registry::Pallet::<T>::register_asset(
		RawOrigin::Root.into(),
		AssetMetadata {
			decimals: MUSD_DECIMALS,
			name: "Mock USD".as_bytes().to_vec(),
			symbol: "MUSD".as_bytes().to_vec(),
			existential_deposit: 0,
			location: None,
			additional: CustomMetadata {
				pool_currency: true,
				..Default::default()
			},
		},
		Some(MUSD_CURRENCY_ID),
	)
	.unwrap();
}

pub fn create_pool<T: Config>(pool_id: PoolId) {
	pallet_pool_registry::Pallet::<T>::register(
		match T::KIND {
			RuntimeKind::Development => RawOrigin::Signed(Account::Admin.id()).into(),
			_ => RawOrigin::Root.into(),
		},
		Account::Admin.id(),
		pool_id,
		vec![
			TrancheInput {
				tranche_type: TrancheType::Residual,
				seniority: None,
				metadata: TrancheMetadata {
					token_name: BoundedVec::default(),
					token_symbol: BoundedVec::default(),
				},
			},
			TrancheInput {
				tranche_type: TrancheType::NonResidual {
					interest_rate_per_sec: One::one(),
					min_risk_buffer: Perquintill::from_percent(10),
				},
				seniority: None,
				metadata: TrancheMetadata {
					token_name: BoundedVec::default(),
					token_symbol: BoundedVec::default(),
				},
			},
		],
		MUSD_CURRENCY_ID,
		POOL_FUNDS,
		None,
		BoundedVec::default(),
	)
	.unwrap();
}

pub fn fund_pool<T: Config>(pool_id: PoolId) {
	let tranche_id = pallet_pool_system::Pool::<T>::get(pool_id)
		.unwrap()
		.tranches
		.tranche_id(TrancheLoc::Index(0))
		.unwrap();

	let role = Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, Moment::MAX));

	pallet_permissions::Pallet::<T>::add(
		RawOrigin::Root.into(),
		role,
		Account::Investor.id(),
		PermissionScope::Pool(pool_id),
		role,
	)
	.unwrap();

	pallet_investments::Pallet::<T>::update_invest_order(
		RawOrigin::Signed(Account::Investor.id()).into(),
		TrancheCurrency::generate(pool_id, tranche_id),
		POOL_FUNDS,
	)
	.unwrap();
}
