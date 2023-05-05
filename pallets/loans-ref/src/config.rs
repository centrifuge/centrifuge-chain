use cfg_traits::data::{DataCollection, DataRegistry};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::marker::PhantomData;

use crate::{
	pallet::Config,
	types::{PoolIdOf, PriceResultOf},
};

const DEFAULT_ERR: DispatchError = DispatchError::Other("No price registry for pallet-loans");

pub struct NoPriceRegistry<T>(std::marker::PhantomData<T>);

impl<T: Config> DataRegistry<T::PriceId, PoolIdOf<T>> for NoPriceRegistry<T> {
	type Collection = NoPriceCollection<T>;
	type Data = PriceResultOf<T>;

	fn get(_: &T::PriceId) -> Self::Data {
		Err(DEFAULT_ERR)
	}

	fn collection(_: &PoolIdOf<T>) -> Self::Collection {
		NoPriceCollection(PhantomData::default())
	}

	fn register_id(_: &T::PriceId, _: &PoolIdOf<T>) -> DispatchResult {
		Err(DEFAULT_ERR)
	}

	fn unregister_id(_: &T::PriceId, _: &PoolIdOf<T>) -> DispatchResult {
		Err(DEFAULT_ERR)
	}
}

pub struct NoPriceCollection<T>(PhantomData<T>);

impl<T: Config> DataCollection<T::PriceId> for NoPriceCollection<T> {
	type Data = PriceResultOf<T>;

	fn get(&self, _: &T::PriceId) -> Self::Data {
		Err(DEFAULT_ERR)
	}
}
