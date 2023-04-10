#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::accrual::{RateAccrual, RateCache};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type OuterRate;
		type AccRate;
		type Moment;
		type Cache: RateCache<Self::OuterRate, Self::AccRate>;
		type MaxRateCount: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type CallIds<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<Blake2_128 as frame_support::StorageHasher>::Output,
		mock_builder::CallId,
	>;

	impl<T: Config> Pallet<T> {
		pub fn mock_accrual(
			f: impl Fn(T::OuterRate) -> Result<T::AccRate, DispatchError> + 'static,
		) {
			register_call!(f);
		}

		pub fn mock_accrual_at(
			f: impl Fn(T::OuterRate, T::Moment) -> Result<T::AccRate, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_last_updated(f: impl Fn() -> T::Moment + 'static) {
			register_call!(move |()| f());
		}

		pub fn mock_validate_rate(f: impl Fn(T::OuterRate) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_reference_rate(f: impl Fn(T::OuterRate) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_unreference_rate(f: impl Fn(T::OuterRate) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_cache(f: impl Fn() -> T::Cache + 'static) {
			register_call!(move |()| f());
		}
	}

	impl<T: Config> RateAccrual for Pallet<T> {
		type AccRate = T::AccRate;
		type Cache = T::Cache;
		type MaxRateCount = T::MaxRateCount;
		type Moment = T::Moment;
		type OuterRate = T::OuterRate;

		fn accrual(a: T::OuterRate) -> Result<T::AccRate, DispatchError> {
			execute_call!(a)
		}

		fn accrual_at(a: T::OuterRate, b: T::Moment) -> Result<T::AccRate, DispatchError> {
			execute_call!((a, b))
		}

		fn last_updated() -> T::Moment {
			execute_call!(())
		}

		fn validate_rate(a: T::OuterRate) -> DispatchResult {
			execute_call!(a)
		}

		fn reference_rate(a: T::OuterRate) -> DispatchResult {
			execute_call!(a)
		}

		fn unreference_rate(a: T::OuterRate) -> DispatchResult {
			execute_call!(a)
		}

		fn cache() -> T::Cache {
			execute_call!(())
		}
	}
}
