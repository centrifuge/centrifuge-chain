use frame_support::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::tokens::CurrencyId;

/// [ISIN](https://en.wikipedia.org/wiki/International_Securities_Identification_Number) format.
pub type Isin = [u8; 12];

/// A representation of an oracle price identifier
#[derive(
	Encode,
	Decode,
	Clone,
	Copy,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	TypeInfo,
	RuntimeDebug,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum OracleKey {
	/// Identify a Isin price
	Isin(Isin),

	/// Identify a conversion from the first currency to the second one
	ConversionRatio(CurrencyId, CurrencyId),
}

impl From<(CurrencyId, CurrencyId)> for OracleKey {
	fn from((from, to): (CurrencyId, CurrencyId)) -> Self {
		Self::ConversionRatio(from, to)
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl From<u32> for OracleKey {
	fn from(value: u32) -> Self {
		// Any u32 value always fits into 12 bytes
		let value_to_array = &(value as u128).to_le_bytes()[0..12];
		let isin = Isin::try_from(value_to_array).unwrap();
		OracleKey::Isin(isin)
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl Default for OracleKey {
	fn default() -> Self {
		OracleKey::Isin(Default::default())
	}
}
