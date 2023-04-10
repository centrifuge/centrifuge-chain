#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::accrual::{AccrualRate, InterestAccrual, RateCollection};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type OuterRate;
		type InnerRate;
		type Moment;
		type Cache: RateCollection;
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
		pub fn mock_accrual_rate(
			f: impl Fn(T::OuterRate) -> Result<AccrualRate<T::InnerRate>, DispatchError> + 'static,
		) {
			register_call!(f);
		}

		pub fn mock_last_updated(f: impl Fn() -> T::Moment + 'static) {
			register_call!(move |()| f());
		}

		pub fn mock_validate(f: impl Fn(T::OuterRate) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_reference(f: impl Fn(T::OuterRate) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_unreference(f: impl Fn(T::OuterRate) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_cache(f: impl Fn() -> T::Cache + 'static) {
			register_call!(move |()| f());
		}
	}

	impl<T: Config> RateCollection for Pallet<T> {
		type InnerRate = T::InnerRate;
		type Moment = T::Moment;
		type OuterRate = T::OuterRate;

		fn accrual_rate(a: T::OuterRate) -> Result<AccrualRate<T::InnerRate>, DispatchError> {
			execute_call!(a)
		}

		fn last_updated() -> T::Moment {
			execute_call!(())
		}
	}

	impl<T: Config> InterestAccrual for Pallet<T> {
		type Cache = T::Cache;

		fn validate(a: T::OuterRate) -> DispatchResult {
			execute_call!(a)
		}

		fn reference(a: T::OuterRate) -> DispatchResult {
			execute_call!(a)
		}

		fn unreference(a: T::OuterRate) -> DispatchResult {
			execute_call!(a)
		}

		fn cache() -> T::Cache {
			execute_call!(())
		}
	}
}
