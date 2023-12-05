use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

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
	Isin(Isin),
}

#[cfg(feature = "runtime-benchmarks")]
impl From<u32> for OracleKey {
	fn from(value: u32) -> Self {
		// Any u32 value always fits into 12 bytes
		let isin = Isin::try_from(&(value as u128).to_be_bytes()[0..12]).unwrap();
		OracleKey::Isin(isin)
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl Default for OracleKey {
	fn default() -> Self {
		OracleKey::Isin(Default::default())
	}
}
