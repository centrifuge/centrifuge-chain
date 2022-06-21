use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use crate::{PoolId, TrancheId};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	// The Native token, representing AIR in Altair and CFG in Centrifuge.
	Native,

	// A Tranche token
	Tranche(PoolId, TrancheId),

	/// Karura KSM
	KSM,

	/// Acala Dollar
	/// In Altair, it represents AUSD in Kusama;
	/// In Centrifuge, it represents AUSD in Polkadot;
	AUSD,

	/// A foreign asset
	ForeignAsset(ForeignAssetId),
}

pub type ForeignAssetId = u32;

impl Default for CurrencyId {
	fn default() -> Self {
		CurrencyId::Native
	}
}

#[macro_export]
macro_rules! impl_tranche_token {
	() => {
		pub struct TrancheToken<Config>(core::marker::PhantomData<(Config)>);

		impl<Config>
			common_traits::TrancheToken<Config::PoolId, Config::TrancheId, Config::CurrencyId>
			for TrancheToken<Config>
		where
			Config: pallet_pools::Config,
			Config::PoolId: Into<u64>,
			Config::TrancheId: Into<[u8; 16]>,
			Config::CurrencyId: From<CurrencyId>,
		{
			fn tranche_token(
				pool: Config::PoolId,
				tranche: Config::TrancheId,
			) -> Config::CurrencyId {
				CurrencyId::Tranche(pool.into(), tranche.into()).into()
			}
		}
	};
}
