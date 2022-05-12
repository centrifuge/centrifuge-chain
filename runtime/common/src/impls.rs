//! Some configurable implementations as associated type for the substrate runtime.

use super::*;
use codec::{Decode, Encode, MaxEncodedLen};
use common_types::CurrencyId;
use core::marker::PhantomData;
use frame_support::sp_runtime::app_crypto::sp_core::U256;
use frame_support::traits::{Currency, OnUnbalanced};
use frame_support::weights::{
	constants::ExtrinsicBaseWeight, WeightToFeeCoefficient, WeightToFeeCoefficients,
	WeightToFeePolynomial,
};
use frame_system::pallet::Config as SystemConfig;
use pallet_authorship::{Config as AuthorshipConfig, Pallet as Authorship};
use pallet_balances::{Config as BalancesConfig, Pallet as Balances};
use scale_info::TypeInfo;
use smallvec::smallvec;
use sp_arithmetic::Perbill;
use sp_core::H160;
use sp_runtime::traits::Convert;
use sp_std::vec;
use sp_std::vec::Vec;

common_types::impl_tranche_token!();

pub struct DealWithFees<Config>(PhantomData<Config>);
pub type NegativeImbalance<Config> =
	<Balances<Config> as Currency<<Config as SystemConfig>::AccountId>>::NegativeImbalance;
impl<Config> OnUnbalanced<NegativeImbalance<Config>> for DealWithFees<Config>
where
	Config: AuthorshipConfig + BalancesConfig + SystemConfig,
{
	fn on_nonzero_unbalanced(amount: NegativeImbalance<Config>) {
		if let Some(who) = Authorship::<Config>::author() {
			Balances::<Config>::resolve_creating(&who, amount);
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
		let p = super::CENTI_CFG;
		let q = 10 * Balance::from(ExtrinsicBaseWeight::get());

		smallvec!(WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q,
		})
	}
}

/// All data for an instance of an NFT.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug, TypeInfo)]
pub struct AssetInfo {
	pub metadata: Bytes,
}

// In order to be generic into T::Address
impl From<Bytes32> for EthAddress {
	fn from(v: Bytes32) -> Self {
		EthAddress(v[..32].try_into().expect("Address wraps a 32 byte array"))
	}
}

impl From<EthAddress> for Bytes32 {
	fn from(a: EthAddress) -> Self {
		a.0
	}
}

impl From<RegistryId> for EthAddress {
	fn from(r: RegistryId) -> Self {
		// Pad 12 bytes to the registry id - total 32 bytes
		let padded = r.0.to_fixed_bytes().iter().copied()
			.chain([0; 12].iter().copied()).collect::<Vec<u8>>()[..32]
			.try_into().expect("RegistryId is 20 bytes. 12 are padded. Converting to a 32 byte array should never fail");

		EthAddress(padded)
	}
}

impl From<EthAddress> for RegistryId {
	fn from(a: EthAddress) -> Self {
		RegistryId(H160::from_slice(&a.0[..20]))
	}
}

impl From<[u8; 20]> for RegistryId {
	fn from(d: [u8; 20]) -> Self {
		RegistryId(H160::from(d))
	}
}

impl AsRef<[u8]> for RegistryId {
	fn as_ref(&self) -> &[u8] {
		self.0.as_ref()
	}
}

impl common_traits::BigEndian<Vec<u8>> for TokenId {
	fn to_big_endian(&self) -> Vec<u8> {
		let mut data = vec![0; 32];
		self.0.to_big_endian(&mut data);
		data
	}
}

impl From<U256> for TokenId {
	fn from(v: U256) -> Self {
		Self(v)
	}
}

impl From<u16> for ItemId {
	fn from(v: u16) -> Self {
		Self(v as u128)
	}
}

impl From<u32> for ItemId {
	fn from(v: u32) -> Self {
		Self(v as u128)
	}
}

impl From<u128> for ItemId {
	fn from(v: u128) -> Self {
		Self(v)
	}
}

impl Convert<TrancheWeight, Balance> for TrancheWeight {
	fn convert(weight: TrancheWeight) -> Balance {
		weight.0
	}
}

impl From<u128> for TrancheWeight {
	fn from(v: u128) -> Self {
		Self(v)
	}
}

/// AssetRegistry's AssetProcessor
pub mod asset_registry {
	use super::*;
	use frame_support::dispatch::RawOrigin;
	use frame_support::sp_std::marker::PhantomData;
	use frame_support::traits::{EnsureOrigin, EnsureOriginWithArg};
	use orml_traits::asset_registry::{AssetMetadata, AssetProcessor};
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
		fn successful_origin(_asset_id: &Option<CurrencyId>) -> Origin {
			unimplemented!()
		}
	}
}

pub mod xcm {
	use crate::{xcm_fees::default_per_second, Balance, CustomMetadata};
	use common_types::CurrencyId;
	use frame_support::sp_std::marker::PhantomData;
	use xcm::latest::MultiLocation;

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
}
