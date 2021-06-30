use codec::{Decode, Encode};
use orml_traits::MultiCurrency;
use pallet_tinlake_investor_pool::Config;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Usd,
	Tranche(u32, u8),
}

pub struct TrancheToken<T>(core::marker::PhantomData<T>);
impl<T> pallet_tinlake_investor_pool::TrancheToken<T> for TrancheToken<T>
where
	T: Config,
	<T as Config>::PoolId: Into<u32>,
	<T as Config>::TrancheId: Into<u8>,
	<<T as Config>::Tokens as MultiCurrency<T::AccountId>>::CurrencyId: From<CurrencyId>,
{
	fn tranche_token(
		pool: <T as Config>::PoolId,
		tranche: <T as Config>::TrancheId,
	) -> <<T as Config>::Tokens as MultiCurrency<T::AccountId>>::CurrencyId {
		CurrencyId::Tranche(pool.into(), tranche.into()).into()
	}
}
