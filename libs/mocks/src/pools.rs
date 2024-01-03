#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{
		investments::InvestmentAccountant, PoolInspect, PoolReserve, PriceValue, Seconds,
		TrancheTokenPrice,
	};
	use cfg_types::investments::InvestmentInfo;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
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

		pub fn mock_info(
			f: impl Fn(
					T::TrancheCurrency,
				) -> Result<
					InvestmentInfo<T::AccountId, T::CurrencyId, T::TrancheCurrency>,
					DispatchError,
				> + 'static,
		) {
			register_call!(f);
		}

		pub fn mock_balance(f: impl Fn(T::TrancheCurrency, &T::AccountId) -> T::Balance + 'static) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_transfer(
			f: impl Fn(T::TrancheCurrency, &T::AccountId, &T::AccountId, T::Balance) -> DispatchResult
				+ 'static,
		) {
			register_call!(move |(a, b, c, d)| f(a, b, c, d));
		}

		#[allow(non_snake_case)]
		pub fn mock_InvestmentAccountant_deposit(
			f: impl Fn(&T::AccountId, T::TrancheCurrency, T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		#[allow(non_snake_case)]
		pub fn mock_InvestmentAccountant_withdraw(
			f: impl Fn(&T::AccountId, T::TrancheCurrency, T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		#[cfg(feature = "runtime-benchmarks")]
		pub fn mock_bench_default_investment_id(
			f: impl Fn(T::PoolId) -> T::TrancheCurrency + 'static,
		) {
			register_call!(f);
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

	impl<T: Config> InvestmentAccountant<T::AccountId> for Pallet<T> {
		type Amount = T::Balance;
		type Error = DispatchError;
		type InvestmentId = T::TrancheCurrency;
		type InvestmentInfo = InvestmentInfo<T::AccountId, T::CurrencyId, Self::InvestmentId>;

		fn info(a: Self::InvestmentId) -> Result<Self::InvestmentInfo, DispatchError> {
			execute_call!(a)
		}

		fn balance(a: Self::InvestmentId, b: &T::AccountId) -> Self::Amount {
			execute_call!((a, b))
		}

		fn transfer(
			a: Self::InvestmentId,
			b: &T::AccountId,
			c: &T::AccountId,
			d: Self::Amount,
		) -> DispatchResult {
			execute_call!((a, b, c, d))
		}

		fn deposit(a: &T::AccountId, b: Self::InvestmentId, c: Self::Amount) -> DispatchResult {
			execute_call!((a, b, c))
		}

		fn withdraw(a: &T::AccountId, b: Self::InvestmentId, c: Self::Amount) -> DispatchResult {
			execute_call!((a, b, c))
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
	impl<T: Config> cfg_traits::benchmarking::PoolBenchmarkHelper for Pallet<T> {
		type AccountId = T::AccountId;
		type PoolId = T::PoolId;

		fn bench_create_pool(_: Self::PoolId, _: &Self::AccountId) {}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> cfg_traits::benchmarking::FundedPoolBenchmarkHelper for Pallet<T> {
		type AccountId = T::AccountId;
		type Balance = T::Balance;
		type PoolId = T::PoolId;

		fn bench_create_funded_pool(_: Self::PoolId, _: &Self::AccountId) {}

		fn bench_investor_setup(_: Self::PoolId, _: Self::AccountId, _: Self::Balance) {}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> cfg_traits::benchmarking::InvestmentIdBenchmarkHelper for Pallet<T> {
		type InvestmentId = T::TrancheCurrency;
		type PoolId = T::PoolId;

		fn bench_default_investment_id(a: Self::PoolId) -> Self::InvestmentId {
			execute_call!(a)
		}
	}
}
