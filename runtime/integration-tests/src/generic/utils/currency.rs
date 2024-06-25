//! PLEASE be as much generic as possible because no domain or use cases are
//! considered at this level.

use cfg_primitives::{conversion, liquidity_pools::GeneralCurrencyPrefix, Balance, CFG};
use cfg_types::tokens::{AssetMetadata, CurrencyId, CustomMetadata, GeneralCurrencyIndex};
use frame_support::{assert_ok, traits::OriginTrait};
use sp_runtime::FixedPointNumber;
use staging_xcm::VersionedLocation;

use crate::generic::config::Runtime;

const fn amount_pow(amount: Balance, exp: u32) -> Balance {
	amount * 10u128.pow(exp)
}

pub const fn cfg(amount: u32) -> Balance {
	amount as Balance * CFG
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

	fn location(&self) -> Option<VersionedLocation> {
		None
	}

	fn custom(&self) -> CustomMetadata {
		CustomMetadata::default()
	}

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

	fn general_currency_index(&self) -> Option<GeneralCurrencyIndex<u128, GeneralCurrencyPrefix>> {
		TryFrom::try_from(*&self.id()).ok()
	}
}

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
			..Default::default()
		}
	}
}

pub const fn usd6(amount: Balance) -> Balance {
	amount_pow(amount, 6)
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
			..Default::default()
		}
	}
}

pub const fn usd12(amount: Balance) -> Balance {
	amount_pow(amount, 12)
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
			..Default::default()
		}
	}
}

pub const fn usd18(amount: Balance) -> Balance {
	amount_pow(amount, 18)
}

#[derive(Clone)]
pub struct CustomCurrency(pub CurrencyId, pub AssetMetadata);
impl CurrencyInfo for CustomCurrency {
	fn id(&self) -> CurrencyId {
		self.0
	}

	fn decimals(&self) -> u32 {
		self.1.decimals
	}

	fn symbol(&self) -> &'static str {
		format!("Custom-{}", self.decimals()).leak()
	}

	fn location(&self) -> Option<VersionedLocation> {
		self.1.location.clone()
	}

	fn custom(&self) -> CustomMetadata {
		self.1.additional
	}

	fn metadata(&self) -> AssetMetadata {
		self.1.clone()
	}
}

impl CustomCurrency {
	// Using `u32` on purpose here to avoid placing a `Balance` as input and
	// generating more decimals than expected.
	pub const fn val(&self, amount: u32) -> Balance {
		amount_pow(amount as Balance, self.1.decimals)
	}

	pub fn metadata(&self) -> &AssetMetadata {
		&self.1
	}
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
