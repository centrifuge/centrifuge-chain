//! Some configurable implementations as associated type for the substrate runtime.

use cfg_types::tokens::CurrencyId;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	sp_runtime::app_crypto::sp_core::U256,
	traits::{Currency, Imbalance, OnUnbalanced},
	weights::{
		constants::ExtrinsicBaseWeight, WeightToFeeCoefficient, WeightToFeeCoefficients,
		WeightToFeePolynomial,
	},
};
use scale_info::TypeInfo;
use smallvec::smallvec;
use sp_arithmetic::Perbill;
use sp_core::H160;
use sp_runtime::traits::Convert;
use sp_std::{vec, vec::Vec};

use super::{
	constants::{CENTI_CFG, TREASURY_FEE_RATIO},
	*,
};

pub mod fees {
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

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, frame_system::MaximumBlockWeight]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	///
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;

		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			let p = CENTI_CFG;
			let q = 10 * Balance::from(ExtrinsicBaseWeight::get());

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
	use frame_support::{
		dispatch::RawOrigin,
		sp_std::marker::PhantomData,
		traits::{EnsureOrigin, EnsureOriginWithArg},
	};
	use orml_traits::asset_registry::{AssetMetadata, AssetProcessor};
	use sp_runtime::DispatchError;

	use super::*;

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
		fn successful_origin(_asset_id: &Option<CurrencyId>) -> Origin {
			unimplemented!()
		}
	}
}

pub mod xcm {
	use cfg_types::tokens::CurrencyId;
	use frame_support::sp_std::marker::PhantomData;
	use sp_runtime::{traits::ConstU32, WeakBoundedVec};
	use xcm::latest::{Junction::GeneralKey, MultiLocation};

	use crate::{xcm_fees::default_per_second, Balance, CustomMetadata};

	/// Our FixedConversionRateProvider, used to charge XCM-related fees for tokens registered in
	/// the asset registry that were not already handled by native Trader rules.
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
			let metadata = OrmlAssetRegistry::metadata_by_location(&location)?;
			metadata
				.additional
				.xcm
				.fee_per_second
				.or_else(|| Some(default_per_second(metadata.decimals)))
		}
	}

	pub fn general_key(key: &[u8]) -> xcm::latest::Junction {
		GeneralKey {

		}
	}
}
