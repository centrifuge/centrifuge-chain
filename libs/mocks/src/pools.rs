#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::{
		investments::InvestmentAccountant, PoolInspect, PoolMutate, PoolReserve, Seconds,
		TrancheTokenPrice, UpdateState,
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
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

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
					(T::PoolId, T::TrancheId),
				) -> Result<
					InvestmentInfo<T::AccountId, T::CurrencyId, (T::PoolId, T::TrancheId)>,
					DispatchError,
				> + 'static,
		) {
			register_call!(f);
		}

		pub fn mock_balance(
			f: impl Fn((T::PoolId, T::TrancheId), &T::AccountId) -> T::Balance + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_transfer(
			f: impl Fn(
					(T::PoolId, T::TrancheId),
					&T::AccountId,
					&T::AccountId,
					T::Balance,
				) -> DispatchResult
				+ 'static,
		) {
			register_call!(move |(a, b, c, d)| f(a, b, c, d));
		}

		pub fn mock_get_price(
			f: impl Fn(T::PoolId, T::TrancheId) -> Option<(T::BalanceRatio, Seconds)> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		#[allow(non_snake_case)]
		pub fn mock_InvestmentAccountant_deposit(
			f: impl Fn(&T::AccountId, (T::PoolId, T::TrancheId), T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		#[allow(non_snake_case)]
		pub fn mock_InvestmentAccountant_withdraw(
			f: impl Fn(&T::AccountId, (T::PoolId, T::TrancheId), T::Balance) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		#[cfg(feature = "runtime-benchmarks")]
		pub fn mock_bench_default_investment_id(
			f: impl Fn(T::PoolId) -> (T::PoolId, T::TrancheId) + 'static,
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
		type InvestmentId = (T::PoolId, T::TrancheId);
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

		fn get_price(a: T::PoolId, b: T::TrancheId) -> Option<(T::BalanceRatio, Seconds)> {
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
		type InvestmentId = (T::PoolId, T::TrancheId);
		type PoolId = T::PoolId;

		fn bench_default_investment_id(a: Self::PoolId) -> Self::InvestmentId {
			execute_call!(a)
		}
	}

	/// Mutability capabilities to this mock
	pub trait ConfigMut: Config {
		type TrancheInput: Encode + Decode + Clone + TypeInfo + Debug + PartialEq;
		type PoolChanges: Encode + Decode + Clone + TypeInfo + Debug + PartialEq + MaxEncodedLen;
		type PoolFeeInput: Encode + Decode + Clone + TypeInfo;
		type MaxTranches: Get<u32>;
		type MaxFeesPerPool: Get<u32>;
	}

	impl<T: ConfigMut> Pallet<T> {
		pub fn mock_create(
			func: impl Fn(
					T::AccountId,
					T::AccountId,
					T::PoolId,
					BoundedVec<T::TrancheInput, T::MaxTranches>,
					T::CurrencyId,
					T::Balance,
					BoundedVec<T::PoolFeeInput, T::MaxFeesPerPool>,
				) -> DispatchResult
				+ 'static,
		) {
			register_call!(move |(a, b, c, d, e, f, g)| func(a, b, c, d, e, f, g));
		}

		pub fn mock_update(
			f: impl Fn(T::PoolId, T::PoolChanges) -> Result<UpdateState, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_execute_update(f: impl Fn(T::PoolId) -> Result<u32, DispatchError> + 'static) {
			register_call!(f);
		}

		#[cfg(feature = "runtime-benchmarks")]
		pub fn mock_worst_pool_changes(f: impl Fn(Option<u32>) -> T::PoolChanges + 'static) {
			register_call!(f);
		}
	}

	impl<T: ConfigMut> PoolMutate<T::AccountId, T::PoolId> for Pallet<T> {
		type Balance = T::Balance;
		type CurrencyId = T::CurrencyId;
		type MaxFeesPerPool = T::MaxFeesPerPool;
		type MaxTranches = T::MaxTranches;
		type PoolChanges = T::PoolChanges;
		type PoolFeeInput = T::PoolFeeInput;
		type TrancheInput = T::TrancheInput;

		fn create(
			a: T::AccountId,
			b: T::AccountId,
			c: T::PoolId,
			d: BoundedVec<T::TrancheInput, T::MaxTranches>,
			e: T::CurrencyId,
			f: T::Balance,
			g: BoundedVec<T::PoolFeeInput, T::MaxFeesPerPool>,
		) -> DispatchResult {
			execute_call!((a, b, c, d, e, f, g))
		}

		fn update(a: T::PoolId, b: T::PoolChanges) -> Result<UpdateState, DispatchError> {
			execute_call!((a, b))
		}

		fn execute_update(a: T::PoolId) -> Result<u32, DispatchError> {
			execute_call!(a)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn worst_pool_changes(a: Option<u32>) -> Self::PoolChanges {
			execute_call!(a)
		}
	}
}
