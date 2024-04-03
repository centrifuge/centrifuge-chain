use cfg_primitives::{
	conversion::fixed_point_to_balance,
	types::{AccountId, Balance, PoolId},
};
use cfg_traits::{HasLocalAssetRepresentation, Millis, PoolInspect, ValueProvider};
use cfg_types::{
	fixed_point::{Quantity, Ratio},
	oracles::OracleKey,
	tokens::{CurrencyId, CustomMetadata},
};
use frame_support::{traits::OriginTrait, RuntimeDebugNoBound};
use orml_traits::asset_registry;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::traits::One;
use sp_runtime::{traits::EnsureInto, DispatchError};
use sp_std::marker::PhantomData;

#[derive(Clone, RuntimeDebugNoBound, TypeInfo, Encode, Decode, MaxEncodedLen)]
#[scale_info(skip_type_params(O))]
pub struct Feeder<O: OriginTrait>(pub O::PalletsOrigin);

impl<O: OriginTrait<AccountId = AccountId>> Feeder<O> {
	pub fn signed(account: AccountId) -> Self {
		Self(O::signed(account).into_caller())
	}

	pub fn root() -> Self {
		Self(O::root().into_caller())
	}

	pub fn none() -> Self {
		Self(O::none().into_caller())
	}
}

impl<O: OriginTrait> PartialEq for Feeder<O> {
	fn eq(&self, other: &Self) -> bool {
		self.0.eq(&other.0)
	}
}

impl<O: OriginTrait> Eq for Feeder<O> {}

impl<O: OriginTrait> PartialOrd for Feeder<O> {
	fn partial_cmp(&self, other: &Self) -> Option<sp_std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl<O: OriginTrait> Ord for Feeder<O> {
	fn cmp(&self, other: &Self) -> sp_std::cmp::Ordering {
		// Since the inner object could not be Ord,
		// we compare their encoded representations
		self.0.encode().cmp(&other.0.encode())
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<O: OriginTrait<AccountId = AccountId>> From<u32> for Feeder<O> {
	fn from(value: u32) -> Self {
		Self(O::signed(frame_benchmarking::account("feeder", value, 0)).into_caller())
	}
}

/// Get the decimals for the pool currency
pub fn decimals_for_pool<Pools, AssetRegistry>(pool_id: PoolId) -> Result<u32, DispatchError>
where
	Pools: PoolInspect<AccountId, CurrencyId, PoolId = PoolId>,
	AssetRegistry: asset_registry::Inspect<AssetId = CurrencyId, CustomMetadata = CustomMetadata>,
{
	let currency = Pools::currency_for(pool_id).ok_or(DispatchError::Other(
		"OracleConverterBridge: No currency for pool",
	))?;

	let metadata = AssetRegistry::metadata(&currency).ok_or(DispatchError::Other(
		"OracleConverterBridge: No metadata for currency",
	))?;

	Ok(metadata.decimals)
}

/// A provider bridge that transform generic quantity representation of a price
/// into a balance denominated in a pool currency.
pub struct OracleConverterBridge<Origin, Provider, Pools, AssetRegistry>(
	PhantomData<(Origin, Provider, Pools, AssetRegistry)>,
);

impl<Origin, Provider, Pools, AssetRegistry> ValueProvider<(Feeder<Origin>, PoolId), OracleKey>
	for OracleConverterBridge<Origin, Provider, Pools, AssetRegistry>
where
	Origin: OriginTrait,
	Provider: ValueProvider<Origin, OracleKey, Value = (Quantity, Millis)>,
	Pools: PoolInspect<AccountId, CurrencyId, PoolId = PoolId>,
	AssetRegistry: asset_registry::Inspect<AssetId = CurrencyId, CustomMetadata = CustomMetadata>,
{
	type Value = (Balance, Millis);

	fn get(
		(feeder, pool_id): &(Feeder<Origin>, PoolId),
		key: &OracleKey,
	) -> Result<Option<Self::Value>, DispatchError> {
		match Provider::get(&feeder.0.clone().into(), key)? {
			Some((quantity, timestamp)) => {
				let decimals =
					decimals_for_pool::<Pools, AssetRegistry>(*pool_id)?.ensure_into()?;
				let balance = fixed_point_to_balance(quantity, decimals)?;

				Ok(Some((balance, timestamp)))
			}
			None => Ok(None),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn set(
		(feeder, pool_id): &(Feeder<Origin>, PoolId),
		key: &OracleKey,
		(balance, timestamp): (Balance, Millis),
	) {
		use cfg_primitives::conversion::balance_to_fixed_point;

		let decimals = decimals_for_pool::<Pools, AssetRegistry>(*pool_id)
			.unwrap()
			.ensure_into()
			.unwrap();

		let fixed_point = balance_to_fixed_point(balance, decimals).unwrap();

		Provider::set(&feeder.0.clone().into(), key, (fixed_point, timestamp));
	}
}

/// A provider to get ratio values from currency pairs
pub struct OracleRatioProvider<Origin, Provider>(PhantomData<(Origin, Provider)>);

impl<Origin, Provider> ValueProvider<Feeder<Origin>, (CurrencyId, CurrencyId)>
	for OracleRatioProvider<Origin, Provider>
where
	Origin: OriginTrait,
	Provider: ValueProvider<Origin, OracleKey, Value = (Ratio, Millis)>,
{
	type Value = Ratio;

	fn get(
		feeder: &Feeder<Origin>,
		(from, to): &(CurrencyId, CurrencyId),
	) -> Result<Option<Self::Value>, DispatchError> {
		Ok(Provider::get(
			&feeder.0.clone().into(),
			&OracleKey::ConversionRatio(*from, *to),
		)?
		.map(|(ratio, _)| ratio))
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn set(feeder: &Feeder<Origin>, (from, to): &(CurrencyId, CurrencyId), ratio: Ratio) {
		Provider::set(
			&feeder.0.clone().into(),
			&OracleKey::ConversionRatio(*from, *to),
			(ratio, 0),
		);
	}
}

/// An extension of the [OracleRatioProvider] which performs a pre-check when
/// querying a feeder key value pair.
pub struct OracleRatioProviderLocalAssetExtension<Origin, Provider, AssetInspect>(
	PhantomData<(Origin, Provider, AssetInspect)>,
);
impl<Origin, Provider, AssetInspect> ValueProvider<Feeder<Origin>, (CurrencyId, CurrencyId)>
	for OracleRatioProviderLocalAssetExtension<Origin, Provider, AssetInspect>
where
	Origin: OriginTrait,
	Provider: ValueProvider<Feeder<Origin>, (CurrencyId, CurrencyId), Value = Ratio>,
	CurrencyId: HasLocalAssetRepresentation<AssetInspect>,
	AssetInspect: asset_registry::Inspect<
		AssetId = CurrencyId,
		Balance = Balance,
		CustomMetadata = CustomMetadata,
	>,
{
	type Value = Ratio;

	fn get(
		feeder: &Feeder<Origin>,
		(from, to): &(CurrencyId, CurrencyId),
	) -> Result<Option<Self::Value>, DispatchError> {
		let locally_coupled_assets = match (from, to) {
			(_, &CurrencyId::LocalAsset(_)) => to.is_local_representation_of(from),
			(&CurrencyId::LocalAsset(_), _) => from.is_local_representation_of(to),
			_ => Ok(false),
		}?;

		if locally_coupled_assets {
			Ok(Some(Ratio::one()))
		} else {
			Provider::get(feeder, &(*from, *to))
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn set(feeder: &Feeder<Origin>, (from, to): &(CurrencyId, CurrencyId), ratio: Ratio) {
		Provider::set(&feeder, &(*from, *to), ratio);
	}
}
