pub use pallet_mock_pools::*;

#[allow(dead_code)]
#[frame_support::pallet]
mod pallet_mock_pools {
	use cfg_primitives::Moment;
	use cfg_traits::{PoolInspect, PoolReserve, PriceValue};
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_std::fmt::Debug;

	use super::super::builder::CallId;
	use crate::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type PoolId: Parameter
			+ Member
			+ Debug
			+ Copy
			+ Default
			+ TypeInfo
			+ Encode
			+ Decode
			+ MaxEncodedLen;
		type TrancheId: Parameter + Member + Debug + Copy + Default + TypeInfo + MaxEncodedLen;
		type Balance;
		type Rate;
		type CurrencyId;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type CallIds<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<Blake2_128 as frame_support::StorageHasher>::Output,
		CallId,
	>;

	impl<T: Config> Pallet<T> {
		pub fn pool_exists_for(f: impl Fn(T::PoolId) -> bool + 'static) {
			register_call!(f);
		}

		pub fn expect_account_for(f: impl Fn(T::PoolId) -> T::AccountId + 'static) {
			register_call!(f);
		}

		pub fn expect_withdraw(
			f: impl Fn(T::PoolId, T::AccountId, T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn expect_deposit(
			f: impl Fn(T::PoolId, T::AccountId, T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}
	}

	impl<T: Config> PoolInspect<T::AccountId, T::CurrencyId> for Pallet<T> {
		type Moment = Moment;
		type PoolId = T::PoolId;
		type Rate = T::Rate;
		type TrancheId = T::TrancheId;

		fn pool_exists(pool_id: T::PoolId) -> bool {
			execute_call!(pool_id)
		}

		fn tranche_exists(_: T::PoolId, _: T::TrancheId) -> bool {
			unimplemented!()
		}

		fn get_tranche_token_price(
			_: T::PoolId,
			_: T::TrancheId,
		) -> Option<PriceValue<T::CurrencyId, T::Rate, Moment>> {
			unimplemented!()
		}

		fn account_for(pool_id: T::PoolId) -> T::AccountId {
			execute_call!(pool_id)
		}
	}

	impl<T: Config> PoolReserve<T::AccountId, T::CurrencyId> for Pallet<T> {
		type Balance = T::Balance;

		fn withdraw(pool_id: T::PoolId, to: T::AccountId, amount: T::Balance) -> DispatchResult {
			execute_call!((pool_id, to, amount))
		}

		fn deposit(pool_id: T::PoolId, from: T::AccountId, amount: T::Balance) -> DispatchResult {
			execute_call!((pool_id, from, amount))
		}
	}
}
