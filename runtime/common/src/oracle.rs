use cfg_primitives::{
	conversion::fixed_point_to_balance,
	types::{AccountId, Balance, Moment, PoolId},
};
use cfg_traits::PoolInspect;
use cfg_types::{
	fixed_point::Quantity,
	oracles::OracleKey,
	tokens::{CurrencyId, CustomMetadata},
};
use orml_traits::{asset_registry, CombineData, DataProviderExtended, OnNewData};
use sp_runtime::traits::Zero;
use sp_std::{marker::PhantomData, vec::Vec};

type TimestampedQuantity = orml_oracle::TimestampedValue<Quantity, Moment>;

/// A provider that maps an `TimestampedQuantity` into a tuple
/// `(Balance, Moment)`.
pub struct DataProviderBridge<Oracle, AssetRegistry, Pools>(
	PhantomData<(Oracle, AssetRegistry, Pools)>,
);

impl<Oracle, AssetRegistry, Pools> DataProviderExtended<(OracleKey, PoolId), (Balance, Moment)>
	for DataProviderBridge<Oracle, AssetRegistry, Pools>
where
	Oracle: DataProviderExtended<OracleKey, TimestampedQuantity>,
	AssetRegistry: asset_registry::Inspect<AssetId = CurrencyId, CustomMetadata = CustomMetadata>,
	Pools: PoolInspect<AccountId, CurrencyId, PoolId = PoolId>,
{
	fn get_no_op((key, pool_id): &(OracleKey, PoolId)) -> Option<(Balance, Moment)> {
		let TimestampedQuantity { value, timestamp } = Oracle::get_no_op(key)?;
		let currency = Pools::currency_for(*pool_id)?;
		let decimals = AssetRegistry::metadata(&currency)?.decimals;

		let balance = fixed_point_to_balance(value, decimals as usize).ok()?;

		Some((balance, timestamp))
	}

	fn get_all_values() -> Vec<((OracleKey, PoolId), Option<(Balance, Moment)>)> {
		// Unimplemented.
		//
		// This method is not used by pallet-data-collector and there is no way to
		// implementing it because `PoolId` is not known by the oracle.
		sp_std::vec![]
	}
}

/// Trigger the new data event as a `Balance` type.
pub struct OnNewPrice<Collector>(PhantomData<Collector>);

impl<Collector> OnNewData<AccountId, OracleKey, Quantity> for OnNewPrice<Collector>
where
	Collector: OnNewData<AccountId, OracleKey, Balance>,
{
	fn on_new_data(account_id: &AccountId, key: &OracleKey, _: &Quantity) {
		// An expected user of `OnNewData` trait should never read/trust the `value`
		// parameter of this call, and instead use a `DataProvider` as source of truth
		// to get the real value, that could be modified by it.
		//
		// Tracking issue: https://github.com/open-web3-stack/open-runtime-module-library/issues/937
		// (4 point)
		Collector::on_new_data(account_id, key, &Balance::zero())
	}
}

/// Always choose the last updated value in case of several values.
pub struct LastOracleValue;

#[cfg(not(feature = "runtime-benchmarks"))]
impl CombineData<OracleKey, TimestampedQuantity> for LastOracleValue {
	fn combine_data(
		_: &OracleKey,
		values: Vec<TimestampedQuantity>,
		_: Option<TimestampedQuantity>,
	) -> Option<TimestampedQuantity> {
		values
			.into_iter()
			.max_by(|v1, v2| v1.timestamp.cmp(&v2.timestamp))
	}
}

/// This is used for feeding the oracle from the data-collector in
/// benchmarks.
/// It can be removed once <https://github.com/open-web3-stack/open-runtime-module-library/issues/920> is merged.
#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarks_util {
	use frame_support::traits::SortedMembers;
	use orml_traits::{DataFeeder, DataProvider};
	use sp_runtime::DispatchResult;
	use sp_std::vec::Vec;

	use super::*;

	// This implementation can be removed once:
	// <https://github.com/open-web3-stack/open-runtime-module-library/pull/920> be merged.
	impl<Oracle, AssetRegistry, Pools> DataProvider<OracleKey, Balance>
		for DataProviderBridge<Oracle, AssetRegistry, Pools>
	where
		Oracle: DataProvider<OracleKey, Quantity>,
	{
		fn get(_: &OracleKey) -> Option<Balance> {
			None
		}
	}

	impl<Oracle, AssetRegistry, Pools> DataFeeder<OracleKey, Balance, AccountId>
		for DataProviderBridge<Oracle, AssetRegistry, Pools>
	where
		Oracle: DataFeeder<OracleKey, Quantity, AccountId>,
	{
		fn feed_value(who: AccountId, key: OracleKey, _: Balance) -> DispatchResult {
			Oracle::feed_value(who, key, Default::default())
		}
	}

	impl CombineData<OracleKey, TimestampedQuantity> for LastOracleValue {
		fn combine_data(
			_: &OracleKey,
			_: Vec<TimestampedQuantity>,
			_: Option<TimestampedQuantity>,
		) -> Option<TimestampedQuantity> {
			Some(TimestampedQuantity {
				value: Default::default(),
				timestamp: 0,
			})
		}
	}

	pub struct Members;

	impl SortedMembers<AccountId> for Members {
		fn sorted_members() -> Vec<AccountId> {
			// We do not want members for benchmarking
			Vec::default()
		}

		fn contains(_: &AccountId) -> bool {
			// We want to mock the member permission for benchmark
			// Allowing any member
			true
		}
	}
}
