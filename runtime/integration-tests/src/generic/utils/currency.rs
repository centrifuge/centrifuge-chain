//! PLEASE be as much generic as possible because no domain or use cases are
//! considered at this level.

use cfg_primitives::{conversion, Balance, CFG};
use cfg_types::tokens::{AssetMetadata, CrossChainTransferability, CurrencyId, CustomMetadata};
use frame_support::{assert_ok, traits::OriginTrait};
use sp_runtime::FixedPointNumber;

use crate::generic::config::Runtime;

pub const fn cfg(amount: Balance) -> Balance {
	amount * CFG
}

pub trait CurrencyInfo {
	fn id(&self) -> CurrencyId;

	fn decimals(&self) -> u32 {
		18
	}

	fn unit(&self) -> Balance {
		10u128.pow(self.decimals())
	}

	fn symbol(&self) -> &'static str {
		"TKN"
	}

	fn name(&self) -> &'static str {
		&self.symbol()
	}

	fn location(&self) -> Option<staging_xcm::VersionedLocation> {
		None
	}

	fn custom(&self) -> CustomMetadata;

	fn ed(&self) -> Balance {
		0
	}

	fn metadata(&self) -> AssetMetadata {
		AssetMetadata {
			decimals: self.decimals(),
			name: self.name().as_bytes().to_vec().try_into().unwrap(),
			symbol: self.symbol().as_bytes().to_vec().try_into().unwrap(),
			existential_deposit: self.ed(),
			location: self.location(),
			additional: self.custom(),
		}
	}
}

/// Matches default() but for const support
pub const CONST_DEFAULT_CUSTOM: CustomMetadata = CustomMetadata {
	transferability: CrossChainTransferability::None,
	mintable: false,
	permissioned: false,
	pool_currency: false,
	local_representation: None,
};

pub fn price_to_currency<N: FixedPointNumber<Inner = Balance>>(
	price: N,
	currency_id: impl CurrencyInfo,
) -> Balance {
	conversion::fixed_point_to_balance(price, currency_id.decimals() as usize).unwrap()
}

pub struct Usd6;
impl CurrencyInfo for Usd6 {
	fn id(&self) -> CurrencyId {
		CurrencyId::ForeignAsset(1)
	}

	fn decimals(&self) -> u32 {
		6
	}

	fn symbol(&self) -> &'static str {
		"USD6"
	}

	fn custom(&self) -> CustomMetadata {
		CustomMetadata {
			pool_currency: true,
			..CONST_DEFAULT_CUSTOM
		}
	}
}

pub const fn usd6(amount: Balance) -> Balance {
	amount * 10u128.pow(6)
}

pub struct Usd12;
impl CurrencyInfo for Usd12 {
	fn id(&self) -> CurrencyId {
		CurrencyId::ForeignAsset(2)
	}

	fn decimals(&self) -> u32 {
		12
	}

	fn symbol(&self) -> &'static str {
		"USD12"
	}

	fn custom(&self) -> CustomMetadata {
		CustomMetadata {
			pool_currency: true,
			..CONST_DEFAULT_CUSTOM
		}
	}
}

pub const fn usd12(amount: Balance) -> Balance {
	amount * 10u128.pow(12)
}

pub struct Usd18;
impl CurrencyInfo for Usd18 {
	fn id(&self) -> CurrencyId {
		CurrencyId::ForeignAsset(3)
	}

	fn decimals(&self) -> u32 {
		18
	}

	fn symbol(&self) -> &'static str {
		"USD12"
	}

	fn custom(&self) -> CustomMetadata {
		CustomMetadata {
			pool_currency: true,
			..CONST_DEFAULT_CUSTOM
		}
	}
}

pub const fn usd18(amount: Balance) -> Balance {
	amount * 10u128.pow(18)
}

pub fn register_currency<T: Runtime>(
	currency: impl CurrencyInfo,
	adaptor: impl FnOnce(&mut AssetMetadata),
) {
	let mut meta = currency.metadata();
	adaptor(&mut meta);
	assert_ok!(orml_asset_registry::module::Pallet::<T>::register_asset(
		<T as frame_system::Config>::RuntimeOrigin::root(),
		meta,
		Some(currency.id())
	));
}
