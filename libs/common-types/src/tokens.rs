use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use common_traits::TokenMetadata;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::format_runtime_string;
use sp_std::vec::Vec;

#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Native,
	Usd,
	Tranche(u64, [u8; 16]),
}

impl TokenMetadata for CurrencyId {
	fn name(&self) -> Vec<u8> {
		match self {
			CurrencyId::Native => b"Native currency".to_vec(),
			CurrencyId::Usd => b"USD stable coin".to_vec(),
			CurrencyId::Tranche(pool_id, tranche_id) => format_runtime_string!(
				"Tranche token of pool {} and tranche {:?}",
				pool_id,
				tranche_id,
			)
			.as_ref()
			.to_vec(),
		}
	}

	fn symbol(&self) -> Vec<u8> {
		match self {
			CurrencyId::Native => b"CFG".to_vec(),
			CurrencyId::Usd => b"USD".to_vec(),
			CurrencyId::Tranche(pool_id, tranche_id) => {
				format_runtime_string!("TT:{}:{:?}", pool_id, tranche_id)
					.as_ref()
					.to_vec()
			}
		}
	}

	fn decimals(&self) -> u8 {
		match self {
			CurrencyId::Native => 18,
			CurrencyId::Usd => 12,
			CurrencyId::Tranche(_, _) => 27,
		}
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
