// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! # Common types and primitives used for Centrifuge chain runtime.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

pub mod account_conversion;
pub mod apis;
pub mod evm;
pub mod routers;

#[macro_export]
macro_rules! production_or_benchmark {
	($production:expr, $benchmark:expr) => {{
		if cfg!(feature = "runtime-benchmarks") {
			$benchmark
		} else {
			$production
		}
	}};
}

pub mod xcm_fees {
	use cfg_primitives::{constants::currency_decimals, types::Balance};
	use frame_support::weights::constants::{ExtrinsicBaseWeight, WEIGHT_REF_TIME_PER_SECOND};

	// The fee cost per second for transferring the native token in cents.
	pub fn native_per_second() -> Balance {
		default_per_second(currency_decimals::NATIVE)
	}

	pub fn ksm_per_second() -> Balance {
		default_per_second(currency_decimals::KSM) / 50
	}

	pub fn default_per_second(decimals: u32) -> Balance {
		let base_weight = Balance::from(ExtrinsicBaseWeight::get().ref_time());
		let default_per_second = WEIGHT_REF_TIME_PER_SECOND as u128 / base_weight;
		default_per_second * base_fee(decimals)
	}

	fn base_fee(decimals: u32) -> Balance {
		dollar(decimals)
			// cents
			.saturating_div(100)
			// a tenth of a cent
			.saturating_div(10)
	}

	pub fn dollar(decimals: u32) -> Balance {
		10u128.saturating_pow(decimals)
	}
}

pub mod fees {
	use cfg_primitives::{
		constants::{CENTI_CFG, TREASURY_FEE_RATIO},
		types::Balance,
	};
	use frame_support::{
		traits::{Currency, Imbalance, OnUnbalanced},
		weights::{
			constants::ExtrinsicBaseWeight, WeightToFeeCoefficient, WeightToFeeCoefficients,
			WeightToFeePolynomial,
		},
	};
	use smallvec::smallvec;
	use sp_arithmetic::Perbill;

	pub type NegativeImbalance<R> = <pallet_balances::Pallet<R> as Currency<
		<R as frame_system::Config>::AccountId,
	>>::NegativeImbalance;

	struct ToAuthor<R>(sp_std::marker::PhantomData<R>);
	impl<R> OnUnbalanced<NegativeImbalance<R>> for ToAuthor<R>
	where
		R: pallet_balances::Config + pallet_authorship::Config,
	{
		fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
			if let Some(author) = <pallet_authorship::Pallet<R>>::author() {
				<pallet_balances::Pallet<R>>::resolve_creating(&author, amount);
			}
		}
	}

	pub struct DealWithFees<R>(sp_std::marker::PhantomData<R>);
	impl<R> OnUnbalanced<NegativeImbalance<R>> for DealWithFees<R>
	where
		R: pallet_balances::Config + pallet_treasury::Config + pallet_authorship::Config,
		pallet_treasury::Pallet<R>: OnUnbalanced<NegativeImbalance<R>>,
	{
		fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<R>>) {
			if let Some(fees) = fees_then_tips.next() {
				// for fees, split the destination
				let (treasury_amount, mut author_amount) = fees.ration(
					TREASURY_FEE_RATIO.deconstruct(),
					(Perbill::one() - TREASURY_FEE_RATIO).deconstruct(),
				);
				if let Some(tips) = fees_then_tips.next() {
					// for tips, if any, 100% to author
					tips.merge_into(&mut author_amount);
				}

				use pallet_treasury::Pallet as Treasury;
				<Treasury<R> as OnUnbalanced<_>>::on_unbalanced(treasury_amount);
				<ToAuthor<R> as OnUnbalanced<_>>::on_unbalanced(author_amount);
			}
		}
	}

	/// Handles converting a weight scalar to a fee value, based on the scale
	/// and granularity of the node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, frame_system::MaximumBlockWeight]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some
	/// examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to
	///     be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;

		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			let p = CENTI_CFG;
			let q = 10 * Balance::from(ExtrinsicBaseWeight::get().ref_time());

			smallvec!(WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			})
		}
	}
}

/// AssetRegistry's AssetProcessor
pub mod asset_registry {
	use cfg_primitives::types::{AccountId, Balance};
	use cfg_types::tokens::{CurrencyId, CustomMetadata};
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{
		dispatch::RawOrigin,
		sp_std::marker::PhantomData,
		traits::{EnsureOrigin, EnsureOriginWithArg},
	};
	use orml_traits::asset_registry::{AssetMetadata, AssetProcessor};
	use scale_info::TypeInfo;
	use sp_runtime::DispatchError;

	#[derive(
		Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
	)]
	pub struct CustomAssetProcessor;

	impl AssetProcessor<CurrencyId, AssetMetadata<Balance, CustomMetadata>> for CustomAssetProcessor {
		fn pre_register(
			id: Option<CurrencyId>,
			metadata: AssetMetadata<Balance, CustomMetadata>,
		) -> Result<(CurrencyId, AssetMetadata<Balance, CustomMetadata>), DispatchError> {
			match id {
				Some(id) => Ok((id, metadata)),
				None => Err(DispatchError::Other("asset-registry: AssetId is required")),
			}
		}

		fn post_register(
			_id: CurrencyId,
			_asset_metadata: AssetMetadata<Balance, CustomMetadata>,
		) -> Result<(), DispatchError> {
			Ok(())
		}
	}

	/// The OrmlAssetRegistry::AuthorityOrigin impl
	pub struct AuthorityOrigin<
		// The origin type
		Origin,
		// The default EnsureOrigin impl used to authorize all
		// assets besides tranche tokens.
		DefaultEnsureOrigin,
	>(PhantomData<(Origin, DefaultEnsureOrigin)>);

	impl<
			Origin: Into<Result<RawOrigin<AccountId>, Origin>> + From<RawOrigin<AccountId>>,
			DefaultEnsureOrigin: EnsureOrigin<Origin>,
		> EnsureOriginWithArg<Origin, Option<CurrencyId>> for AuthorityOrigin<Origin, DefaultEnsureOrigin>
	{
		type Success = ();

		fn try_origin(
			origin: Origin,
			asset_id: &Option<CurrencyId>,
		) -> Result<Self::Success, Origin> {
			match asset_id {
				// Only the pools pallet should directly register/update tranche tokens
				Some(CurrencyId::Tranche(_, _)) => Err(origin),

				// Any other `asset_id` defaults to EnsureRoot
				_ => DefaultEnsureOrigin::try_origin(origin).map(|_| ()),
			}
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin(_asset_id: &Option<CurrencyId>) -> Result<Origin, ()> {
			Err(())
		}
	}
}

pub mod xcm {
	use cfg_primitives::types::Balance;
	use cfg_types::tokens::{CrossChainTransferability, CurrencyId, CustomMetadata};
	use frame_support::sp_std::marker::PhantomData;
	use sp_runtime::traits::Convert;
	use xcm::{
		latest::{Junction::GeneralKey, MultiLocation},
		prelude::{AccountId32, X1},
	};

	use crate::xcm_fees::default_per_second;

	/// Our FixedConversionRateProvider, used to charge XCM-related fees for
	/// tokens registered in the asset registry that were not already handled by
	/// native Trader rules.
	pub struct FixedConversionRateProvider<OrmlAssetRegistry>(PhantomData<OrmlAssetRegistry>);

	impl<
			OrmlAssetRegistry: orml_traits::asset_registry::Inspect<
				AssetId = CurrencyId,
				Balance = Balance,
				CustomMetadata = CustomMetadata,
			>,
		> orml_traits::FixedConversionRateProvider for FixedConversionRateProvider<OrmlAssetRegistry>
	{
		fn get_fee_per_second(location: &MultiLocation) -> Option<u128> {
			let metadata = OrmlAssetRegistry::metadata_by_location(location)?;
			match metadata.additional.transferability {
				CrossChainTransferability::Xcm(xcm_metadata)
				| CrossChainTransferability::All(xcm_metadata) => xcm_metadata
					.fee_per_second
					.or_else(|| Some(default_per_second(metadata.decimals))),
				_ => None,
			}
		}
	}

	/// A utils function to un-bloat and simplify the instantiation of
	/// `GeneralKey` values
	pub fn general_key(data: &[u8]) -> xcm::latest::Junction {
		GeneralKey {
			length: data.len().min(32) as u8,
			data: cfg_utils::vec_to_fixed_array(data.to_vec()),
		}
	}

	/// How we convert an `[AccountId]` into an XCM MultiLocation
	pub struct AccountIdToMultiLocation<AccountId>(PhantomData<AccountId>);
	impl<AccountId> Convert<AccountId, MultiLocation> for AccountIdToMultiLocation<AccountId>
	where
		AccountId: Into<[u8; 32]>,
	{
		fn convert(account: AccountId) -> MultiLocation {
			X1(AccountId32 {
				network: None,
				id: account.into(),
			})
			.into()
		}
	}
}

pub mod oracle {
	use cfg_primitives::types::{AccountId, Balance, Moment};
	use cfg_types::oracles::OracleKey;
	use orml_traits::{CombineData, DataFeeder, DataProvider, DataProviderExtended};
	use sp_runtime::DispatchResult;
	use sp_std::{marker::PhantomData, vec::Vec};

	type OracleValue = orml_oracle::TimestampedValue<Balance, Moment>;

	/// Always choose the last updated value in case of several values.
	pub struct LastOracleValue;

	#[cfg(not(feature = "runtime-benchmarks"))]
	impl CombineData<OracleKey, OracleValue> for LastOracleValue {
		fn combine_data(
			_: &OracleKey,
			values: Vec<OracleValue>,
			_: Option<OracleValue>,
		) -> Option<OracleValue> {
			values
				.into_iter()
				.max_by(|v1, v2| v1.timestamp.cmp(&v2.timestamp))
		}
	}

	/// A provider that maps an `OracleValue` into a tuple `(Balance, Moment)`.
	/// This aux type is forced because of <https://github.com/open-web3-stack/open-runtime-module-library/issues/904>
	/// and can be removed once they fix this.
	pub struct DataProviderBridge<OrmlOracle>(PhantomData<OrmlOracle>);

	impl<OrmlOracle: DataProviderExtended<OracleKey, OracleValue>>
		DataProviderExtended<OracleKey, (Balance, Moment)> for DataProviderBridge<OrmlOracle>
	{
		fn get_no_op(key: &OracleKey) -> Option<(Balance, Moment)> {
			OrmlOracle::get_no_op(key).map(|OracleValue { value, timestamp }| (value, timestamp))
		}

		fn get_all_values() -> Vec<(OracleKey, Option<(Balance, Moment)>)> {
			OrmlOracle::get_all_values()
				.into_iter()
				.map(|elem| {
					(
						elem.0,
						elem.1
							.map(|OracleValue { value, timestamp }| (value, timestamp)),
					)
				})
				.collect()
		}
	}

	impl<OrmlOracle: DataProvider<OracleKey, Balance>> DataProvider<OracleKey, Balance>
		for DataProviderBridge<OrmlOracle>
	{
		fn get(key: &OracleKey) -> Option<Balance> {
			OrmlOracle::get(key)
		}
	}

	impl<OrmlOracle: DataFeeder<OracleKey, Balance, AccountId>>
		DataFeeder<OracleKey, Balance, AccountId> for DataProviderBridge<OrmlOracle>
	{
		fn feed_value(who: AccountId, key: OracleKey, value: Balance) -> DispatchResult {
			OrmlOracle::feed_value(who, key, value)
		}
	}

	/// This is used for feeding the oracle from the data-collector in
	/// benchmarks.
	/// It can be removed once <https://github.com/open-web3-stack/open-runtime-module-library/issues/920> is merged.
	#[cfg(feature = "runtime-benchmarks")]
	pub mod benchmarks_util {
		use frame_support::traits::SortedMembers;
		use sp_std::vec::Vec;

		use super::*;

		impl CombineData<OracleKey, OracleValue> for LastOracleValue {
			fn combine_data(
				_: &OracleKey,
				_: Vec<OracleValue>,
				_: Option<OracleValue>,
			) -> Option<OracleValue> {
				Some(OracleValue {
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
}

pub mod changes {
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::RuntimeDebug;
	use pallet_loans::ChangeOf as LoansChangeOf;
	use pallet_pool_system::pool_types::changes::PoolChangeProposal;
	use scale_info::TypeInfo;
	use sp_runtime::DispatchError;

	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum RuntimeChange<T: pallet_loans::Config> {
		Loan(LoansChangeOf<T>),
	}

	#[cfg(not(feature = "runtime-benchmarks"))]
	impl<T: pallet_loans::Config> From<RuntimeChange<T>> for PoolChangeProposal {
		fn from(RuntimeChange::Loan(loans_change): RuntimeChange<T>) -> Self {
			use cfg_primitives::SECONDS_PER_WEEK;
			use pallet_loans::types::{InternalMutation, LoanMutation};
			use pallet_pool_system::pool_types::changes::Requirement;
			use sp_std::vec;

			let epoch = Requirement::NextEpoch;
			let week = Requirement::DelayTime(SECONDS_PER_WEEK as u32);
			let blocked = Requirement::BlockedByLockedRedemptions;

			let requirements = match loans_change {
				// Requirements gathered from
				// <https://docs.google.com/spreadsheets/d/1RJ5RLobAdumXUK7k_ugxy2eDAwI5akvtuqUM2Tyn5ts>
				LoansChangeOf::<T>::Loan(_, loan_mutation) => match loan_mutation {
					LoanMutation::Maturity(_) => vec![week, blocked],
					LoanMutation::MaturityExtension(_) => vec![],
					LoanMutation::InterestPayments(_) => vec![week, blocked],
					LoanMutation::PayDownSchedule(_) => vec![week, blocked],
					LoanMutation::InterestRate(_) => vec![epoch],
					LoanMutation::Internal(mutation) => match mutation {
						InternalMutation::ValuationMethod(_) => vec![week, blocked],
						InternalMutation::ProbabilityOfDefault(_) => vec![epoch],
						InternalMutation::LossGivenDefault(_) => vec![epoch],
						InternalMutation::DiscountRate(_) => vec![epoch],
					},
				},
				LoansChangeOf::<T>::Policy(_) => vec![week, blocked],
			};

			PoolChangeProposal::new(requirements)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: pallet_loans::Config> From<RuntimeChange<T>> for PoolChangeProposal {
		fn from(RuntimeChange::Loan(_): RuntimeChange<T>) -> Self {
			// We dont add any requirement in case of benchmarking.
			// We assume checking requirements in the pool is something very fast and
			// deprecable in relation to reading from any storage.
			// If tomorrow any requirement requires a lot of time,
			// it should be precomputed in any pool stage, to make the requirement
			// validation as fast as possible.
			PoolChangeProposal::new([])
		}
	}

	/// Used for building CfgChanges in pallet-loans
	impl<T: pallet_loans::Config> From<LoansChangeOf<T>> for RuntimeChange<T> {
		fn from(loan_change: LoansChangeOf<T>) -> RuntimeChange<T> {
			RuntimeChange::Loan(loan_change)
		}
	}

	/// Used for recovering LoanChange in pallet-loans
	impl<T: pallet_loans::Config> TryInto<LoansChangeOf<T>> for RuntimeChange<T> {
		type Error = DispatchError;

		fn try_into(self) -> Result<LoansChangeOf<T>, DispatchError> {
			let RuntimeChange::Loan(loan_change) = self;
			Ok(loan_change)
		}
	}
}
