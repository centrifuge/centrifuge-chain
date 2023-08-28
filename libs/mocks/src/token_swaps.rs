#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::TokenSwaps;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type CurrencyId;
		type Balance;
		type SellRatio;
		type OrderId;
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
		pub fn mock_place_order(
			f: impl Fn(
					T::AccountId,
					T::CurrencyId,
					T::CurrencyId,
					T::Balance,
					T::SellRatio,
					T::Balance,
				) -> Result<T::OrderId, DispatchError>
				+ 'static,
		) {
			register_call!(move |(a, b, c, d, e, g)| f(a, b, c, d, e, g));
		}

		pub fn mock_update_order(
			f: impl Fn(T::AccountId, T::OrderId, T::Balance, T::SellRatio, T::Balance) -> DispatchResult
				+ 'static,
		) {
			register_call!(move |(a, b, c, d, e)| f(a, b, c, d, e));
		}

		pub fn mock_cancel_order(f: impl Fn(T::OrderId) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_is_active(f: impl Fn(T::OrderId) -> bool + 'static) {
			register_call!(f);
		}
	}

	impl<T: Config> TokenSwaps<T::AccountId> for Pallet<T> {
		type Balance = T::Balance;
		type CurrencyId = T::CurrencyId;
		type OrderId = T::OrderId;
		type SellRatio = T::SellRatio;

		fn place_order(
			a: T::AccountId,
			b: Self::CurrencyId,
			c: Self::CurrencyId,
			d: Self::Balance,
			e: Self::SellRatio,
			f: Self::Balance,
		) -> Result<Self::OrderId, DispatchError> {
			execute_call!((a, b, c, d, e, f))
		}

		fn update_order(
			a: T::AccountId,
			b: Self::OrderId,
			c: Self::Balance,
			d: Self::SellRatio,
			e: Self::Balance,
		) -> DispatchResult {
			execute_call!((a, b, c, d, e))
		}

		fn cancel_order(a: Self::OrderId) -> DispatchResult {
			execute_call!(a)
		}

		fn is_active(a: Self::OrderId) -> bool {
			execute_call!(a)
		}

		fn order_pair_exists(a: Self::CurrencyId, b: Self::CurrencyId) -> bool {
			execute_call!((a, b))
		}

		fn counter_order_pair_exists(a: Self::CurrencyId, b: Self::CurrencyId) -> bool {
			execute_call!((a, b))
		}
	}
}
