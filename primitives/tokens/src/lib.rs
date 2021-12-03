#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Usd,
	Tranche(u64, u8),
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
