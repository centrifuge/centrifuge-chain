use cfg_primitives::{currency_decimals, Balance, Moment, PoolId, TrancheId, CFG};
use cfg_traits::investments::TrancheCurrency as TrancheCurrencyT;
use cfg_types::{
	consts::pools::{MaxTrancheNameLengthBytes, MaxTrancheSymbolLengthBytes},
	domain_address::{Domain, DomainAddress},
	fixed_point::{Quantity, Rate},
	permissions::{PermissionScope, PoolRole, Role},
	pools::TrancheMetadata,
	tokens::{AssetMetadata, CurrencyId, CustomMetadata, TrancheCurrency},
};
use frame_support::{
	assert_noop,
	traits::{GenesisBuild, Get, OriginTrait},
	BoundedVec,
};
use pallet_pool_system::tranches::{TrancheInput, TrancheLoc, TrancheType};
use sp_runtime::{traits::One, AccountId32, Perquintill};

use super::generic::Config;
use crate::utils::accounts::Keyring;

pub const MUSD_DECIMALS: u32 = 6;
pub const MUSD_UNIT: Balance = 10u128.pow(MUSD_DECIMALS);
pub const MUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(23);

pub const ADMIN: Keyring = Keyring::Alice;
pub const BORROWER: Keyring = Keyring::Bob;
pub const INVESTOR: Keyring = Keyring::Charlie;

pub const POOL_FUNDS: Balance = 100_000_000 * MUSD_UNIT;

pub fn new_ext<T: Config>() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<T>()
		.unwrap();

	pallet_balances::GenesisConfig::<T> {
		balances: vec![
			(ADMIN.to_account_id(), T::PoolDeposit::get()),
			(BORROWER.to_account_id(), 1 * CFG),
			(INVESTOR.to_account_id(), 1 * CFG),
		],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	orml_tokens::GenesisConfig::<T> {
		balances: vec![
			(ADMIN.to_account_id(), MUSD_CURRENCY_ID, 1 * MUSD_UNIT),
			(BORROWER.to_account_id(), MUSD_CURRENCY_ID, 1 * MUSD_UNIT),
			(INVESTOR.to_account_id(), MUSD_CURRENCY_ID, POOL_FUNDS),
		],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	sp_io::TestExternalities::new(storage)
}

pub fn register_usdt<T: Config>() {
	orml_asset_registry::Pallet::<T>::register_asset(
		T::RuntimeOrigin::root(),
		AssetMetadata {
			decimals: MUSD_DECIMALS,
			name: "MOCK USD".as_bytes().to_vec(),
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

/*
pub fn create_pool<T: Config>(pool_id: PoolId) {
	pallet_pool_registry::Pallet::<T>::register(
		T::RuntimeOrigin::signed(ADMIN.to_account_id()),
		ADMIN.to_account_id(),
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
		T::RuntimeOrigin::root(),
		role,
		INVESTOR.to_account_id(),
		PermissionScope::Pool(pool_id),
		role,
	);

	pallet_investments::Pallet::<T>::update_invest_order(
		T::RuntimeOrigin::signed(INVESTOR.into()),
		TrancheCurrency::generate(pool_id, tranche_id),
		POOL_FUNDS,
	);
}
*/
