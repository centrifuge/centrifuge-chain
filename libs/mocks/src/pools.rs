#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{PoolInspect, PoolReserve, PriceValue, Seconds, TrancheTokenPrice};
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};
	use scale_info::TypeInfo;
	use sp_std::fmt::Debug;

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
		type BalanceRatio;
		type CurrencyId;
		type TrancheCurrency;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type CallIds<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<Blake2_128 as frame_support::StorageHasher>::Output,
		mock_builder::CallId,
	>;

	impl<T: Config> Pallet<T> {
		pub fn mock_pool_exists(f: impl Fn(T::PoolId) -> bool + 'static) {
			register_call!(f);
		}

		pub fn mock_tranche_exists(f: impl Fn(T::PoolId, T::TrancheId) -> bool + 'static) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_account_for(f: impl Fn(T::PoolId) -> T::AccountId + 'static) {
			register_call!(f);
		}

		pub fn mock_currency_for(f: impl Fn(T::PoolId) -> Option<T::CurrencyId> + 'static) {
			register_call!(f);
		}

		pub fn mock_withdraw(
			f: impl Fn(T::PoolId, T::AccountId, T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_deposit(
			f: impl Fn(T::PoolId, T::AccountId, T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}
	}

	impl<T: Config> PoolInspect<T::AccountId, T::CurrencyId> for Pallet<T> {
		type Moment = Seconds;
		type PoolId = T::PoolId;
		type TrancheId = T::TrancheId;

		fn pool_exists(a: T::PoolId) -> bool {
			execute_call!(a)
		}

		fn tranche_exists(a: T::PoolId, b: T::TrancheId) -> bool {
			execute_call!((a, b))
		}

		fn account_for(a: T::PoolId) -> T::AccountId {
			execute_call!(a)
		}

		fn currency_for(a: T::PoolId) -> Option<T::CurrencyId> {
			execute_call!(a)
		}
	}

	impl<T: Config> TrancheTokenPrice<T::AccountId, T::CurrencyId> for Pallet<T> {
		type BalanceRatio = T::BalanceRatio;
		type Moment = Seconds;
		type PoolId = T::PoolId;
		type TrancheId = T::TrancheId;

		fn get(
			a: T::PoolId,
			b: T::TrancheId,
		) -> Option<PriceValue<T::CurrencyId, T::BalanceRatio, Seconds>> {
			execute_call!((a, b))
		}
	}

	impl<T: Config> PoolReserve<T::AccountId, T::CurrencyId> for Pallet<T> {
		type Balance = T::Balance;

		fn withdraw(a: T::PoolId, b: T::AccountId, c: T::Balance) -> DispatchResult {
			execute_call!((a, b, c))
		}

		fn deposit(a: T::PoolId, b: T::AccountId, c: T::Balance) -> DispatchResult {
			execute_call!((a, b, c))
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> cfg_traits::benchmarking::FundedPoolBenchmarkHelper for Pallet<T> {
		type AccountId = T::AccountId;
		type Balance = T::Balance;
		type PoolId = T::PoolId;

		fn bench_create_funded_pool(a: Self::PoolId, b: &Self::AccountId) {}

		fn bench_investor_setup(a: Self::PoolId, b: Self::AccountId, c: Self::Balance) {}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> cfg_traits::benchmarking::InvestmentIdBenchmarkHelper for Pallet<T> {
		type InvestmentId = T::TrancheCurrency;
		type PoolId = T::PoolId;

		fn bench_default_investment_id(a: Self::PoolId) -> Self::InvestmentId {}
	}
}
