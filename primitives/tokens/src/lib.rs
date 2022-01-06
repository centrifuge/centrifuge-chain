#![cfg_attr(not(feature = "std"), no_std)]

use core::convert::TryFrom;
use codec::{Decode, Encode};
use scale_info::TypeInfo;


#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Usd,
	Tranche(u64, u8),
}


impl TryFrom<CurrencyId> for u128 {
	type Error = &'static str;

	fn try_from(value: CurrencyId) -> Result<Self, Self::Error> {
		match value {
			CurrencyId::Usd => Ok(0),
			CurrencyId::Tranche(_,_) => Err("CurrencyId::Tranche cannot be converted")
		}
	}
}

impl TryFrom<u128> for CurrencyId {
	type Error = &'static str;

	fn try_from(value: u128) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(CurrencyId::Usd),
			_ => Err("Unsupported u128 representation of CurrencyId")
		}
	}
}

#[macro_export]
macro_rules! impl_tranche_token {
	() => {
		pub struct TrancheToken<T>(core::marker::PhantomData<T>);

		impl<T> pallet_tinlake_investor_pool::TrancheToken<T> for TrancheToken<T>
		where
			T: Config,
			<T as Config>::PoolId: Into<u64>,
			<T as Config>::TrancheId: Into<u8>,
			<T as Config>::CurrencyId: From<CurrencyId>,
		{
			fn tranche_token(
				pool: <T as Config>::PoolId,
				tranche: <T as Config>::TrancheId,
			) -> <T as Config>::CurrencyId {
				CurrencyId::Tranche(pool.into(), tranche.into()).into()
			}
		}
	};
}
