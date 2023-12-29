//! PLEASE be as much generic as possible because no domain or use cases are
//! considered at this level.

use cfg_primitives::{conversion, Balance, CFG};
use cfg_types::{
	tokens::{AssetMetadata, CrossChainTransferability, CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use frame_support::{assert_ok, traits::OriginTrait};
use sp_runtime::FixedPointNumber;
use xcm::VersionedMultiLocation;

use crate::generic::{config::Runtime, envs::fudge_env::FudgeSupport};

pub const fn cfg(amount: Balance) -> Balance {
	amount * CFG
}

pub trait CurrencyInfo {
	const ID: CurrencyId;
	const DECIMALS: u32;
	const UNIT: Balance = 10u128.pow(Self::DECIMALS);
	const SYMBOL: &'static str;
	const NAME: &'static str = Self::SYMBOL;
	const LOCATION: Option<xcm::VersionedMultiLocation> = None;
	const CUSTOM: CustomMetadata;
	const ED: Balance = 0;

	fn metadata() -> AssetMetadata<Balance, CustomMetadata> {
		AssetMetadata {
			decimals: Self::DECIMALS,
			name: Self::NAME.as_bytes().to_vec(),
			symbol: Self::SYMBOL.as_bytes().to_vec(),
			existential_deposit: Self::ED,
			location: None,
			additional: CustomMetadata {
				pool_currency: true,
				..Default::default()
			},
		}
	}
}

/// Matches default() but for const support
const CONST_DEFAULT_CUSTOM: CustomMetadata = CustomMetadata {
	transferability: CrossChainTransferability::None,
	mintable: false,
	permissioned: false,
	pool_currency: false,
};

pub fn find_metadata(currency_id: CurrencyId) -> AssetMetadata<Balance, CustomMetadata> {
	match currency_id {
		Usd6::ID => Usd6::metadata(),
		Usd12::ID => Usd12::metadata(),
		Usd18::ID => Usd18::metadata(),
		_ => panic!("Unsupported currency {currency_id:?}"),
	}
}

pub fn price_to_currency<N: FixedPointNumber<Inner = Balance>>(
	price: N,
	currency_id: CurrencyId,
) -> Balance {
	let decimals = find_metadata(currency_id).decimals;
	conversion::fixed_point_to_balance(price, decimals as usize).unwrap()
}

pub struct Usd6;
impl CurrencyInfo for Usd6 {
	const CUSTOM: CustomMetadata = CustomMetadata {
		pool_currency: true,
		transferability: CrossChainTransferability::Xcm(XcmMetadata {
			fee_per_second: Some(1_000),
		}),
		..CONST_DEFAULT_CUSTOM
	};
	const DECIMALS: u32 = 6;
	const ID: CurrencyId = CurrencyId::ForeignAsset(1);
	const SYMBOL: &'static str = "USD6";
}

pub const fn usd6(amount: Balance) -> Balance {
	amount * Usd6::UNIT
}

pub struct Usd12;
impl CurrencyInfo for Usd12 {
	const CUSTOM: CustomMetadata = CustomMetadata {
		pool_currency: true,
		..CONST_DEFAULT_CUSTOM
	};
	const DECIMALS: u32 = 12;
	const ID: CurrencyId = CurrencyId::ForeignAsset(2);
	const SYMBOL: &'static str = "USD12";
}

pub const fn usd12(amount: Balance) -> Balance {
	amount * Usd12::UNIT
}

pub struct Usd18;
impl CurrencyInfo for Usd18 {
	const CUSTOM: CustomMetadata = CustomMetadata {
		pool_currency: true,
		..CONST_DEFAULT_CUSTOM
	};
	const DECIMALS: u32 = 18;
	const ID: CurrencyId = CurrencyId::ForeignAsset(3);
	const SYMBOL: &'static str = "USD12";
}

pub const fn usd18(amount: Balance) -> Balance {
	amount * Usd18::UNIT
}

pub fn register_currency<T: Runtime + FudgeSupport, C: CurrencyInfo>(
	location: Option<VersionedMultiLocation>,
) {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: C::DECIMALS,
		name: C::NAME.into(),
		symbol: C::SYMBOL.into(),
		existential_deposit: C::ED,
		location,
		additional: C::CUSTOM,
	};

	assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
		<T as frame_system::Config>::RuntimeOrigin::root(),
		meta,
		Some(C::ID)
	));
}
