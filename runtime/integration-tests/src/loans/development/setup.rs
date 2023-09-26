use cfg_primitives::{currency_decimals, Balance, Moment, PoolId, TrancheId, CFG};
use cfg_types::{
	consts::pools::{MaxTrancheNameLengthBytes, MaxTrancheSymbolLengthBytes},
	domain_address::{Domain, DomainAddress},
	fixed_point::{Quantity, Rate},
	pools::TrancheMetadata,
	tokens::{AssetMetadata, CurrencyId, CustomMetadata},
};
use development_runtime::{OrmlAssetRegistry, PoolDeposit, PoolRegistry, Runtime, RuntimeOrigin};
use frame_support::{assert_noop, traits::GenesisBuild, BoundedVec};
use pallet_pool_system::tranches::{TrancheInput, TrancheType};
use sp_runtime::{traits::One, Perquintill};

use crate::utils::accounts::Keyring;

pub const MUSD_DECIMALS: u32 = 6;
pub const MUSD_UNIT: Balance = 10u128.pow(MUSD_DECIMALS);
pub const MUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(23);
pub const ADMIN: Keyring = Keyring::Alice;
pub const BORROWER: Keyring = Keyring::Bob;

pub fn new_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![
			(ADMIN.to_account_id(), PoolDeposit::get()),
			(BORROWER.to_account_id(), 1 * CFG),
		],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	orml_tokens::GenesisConfig::<Runtime> {
		balances: vec![
			(ADMIN.to_account_id(), MUSD_CURRENCY_ID, 1 * MUSD_UNIT),
			(BORROWER.to_account_id(), MUSD_CURRENCY_ID, 1 * MUSD_UNIT),
		],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	sp_io::TestExternalities::new(storage)
}

pub fn register_usdt() {
	OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
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

pub fn create_pool(pool_id: PoolId) {
	PoolRegistry::register(
		RuntimeOrigin::signed(ADMIN.into()),
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
		u32::max_value() as u128,
		None,
		BoundedVec::default(),
	)
	.unwrap();
}
