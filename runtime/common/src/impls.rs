//! Some configurable implementations as associated type for the substrate runtime.

use super::*;
use codec::{Decode, Encode};
use core::marker::PhantomData;
use frame_support::traits::{Currency, OnUnbalanced};
use frame_support::weights::{
	WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use frame_system::pallet::Config as SystemConfig;
use pallet_authorship::{Config as AuthorshipConfig, Pallet as Authorship};
use pallet_balances::{Config as BalancesConfig, Pallet as Balances};
use smallvec::smallvec;
use sp_arithmetic::Perbill;
use sp_core::H160;
use std::convert::TryInto;

pub struct DealWithFees<Config>(PhantomData<Config>);
pub type NegativeImbalance<Config> =
	<Balances<Config> as Currency<<Config as SystemConfig>::AccountId>>::NegativeImbalance;
impl<Config> OnUnbalanced<NegativeImbalance<Config>> for DealWithFees<Config>
where
	Config: AuthorshipConfig + BalancesConfig + SystemConfig,
{
	fn on_nonzero_unbalanced(amount: NegativeImbalance<Config>) {
		Balances::<Config>::resolve_creating(&Authorship::<Config>::author(), amount);
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
/// Sample weight to Fee Calculation for 1 Rad Balance transfer:
/// ```rust
/// 	use node_primitives::Balance;
/// 	let extrinsic_bytes: Balance = 92;
/// 	let weight: Balance = 195000000;
/// 	let weight_coefficient: Balance = 315000;
/// 	let transaction_byte_fee: Balance = 10000000000; // 0.01 Micro RAD
///		let maximum_block_weight: Balance = 2000000000000; // 2 * WEIGHT_PER_SECOND
/// 	let extrinsic_base_weight: Balance = 125000000; // 125 * WEIGHT_PER_MICROS
///
/// 	// Calculation:
/// 	let base_fee: Balance = extrinsic_base_weight * weight_coefficient; // 39375000000000
/// 	let length_fee: Balance = extrinsic_bytes * transaction_byte_fee; // 920000000000
/// 	let weight_fee: Balance = weight * weight_coefficient; // 61425000000000
/// 	let fee: Balance = base_fee + length_fee + weight_fee;
/// 	assert_eq!(fee, 10172 * (centrifuge_chain_runtime::constants::currency::MICRO_AIR / 100));
/// ```
pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;

	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		smallvec!(WeightToFeeCoefficient {
			coeff_integer: 315000,
			coeff_frac: Perbill::zero(),
			negative: false,
			degree: 1,
		})
	}
}

/// All data for an instance of an NFT.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
pub struct AssetInfo {
	pub metadata: Bytes,
}

/// A generic representation of a local address. A resource id points to this. It may be a
/// registry id (20 bytes) or a fungible asset type (in the future). Constrained to 32 bytes just
/// as an upper bound to store efficiently.
#[derive(codec::Encode, codec::Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct EthAddress(pub Bytes32);

impl From<RegistryId> for EthAddress {
	fn from(r: RegistryId) -> Self {
		// Pad 12 bytes to the registry id - total 32 bytes
		let padded = r.to_fixed_bytes().iter().copied()
			.chain([0; 12].iter().copied()).collect::<Vec<u8>>()[..32]
			.try_into().expect("RegistryId is 20 bytes. 12 are padded. Converting to a 32 byte array should never fail");

		EthAddress(padded)
	}
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

impl From<EthAddress> for RegistryId {
	fn from(a: EthAddress) -> Self {
		H160::from_slice(&a.0[..20])
	}
}
