#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{OrderRatio, TokenSwaps};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type CurrencyId;
		type Balance;
		type Ratio;
		type OrderId;
		type OrderDetails;
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
		pub fn mock_place_order(
			f: impl Fn(
					T::AccountId,
					T::CurrencyId,
					T::CurrencyId,
					T::Balance,
					OrderRatio<T::Ratio>,
				) -> Result<T::OrderId, DispatchError>
				+ 'static,
		) {
			register_call!(move |(a, b, c, d, e)| f(a, b, c, d, e));
		}

		pub fn mock_update_order(
			f: impl Fn(T::OrderId, T::Balance, OrderRatio<T::Ratio>) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_cancel_order(f: impl Fn(T::OrderId) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_is_active(f: impl Fn(T::OrderId) -> bool + 'static) {
			register_call!(f);
		}

		pub fn mock_valid_pair(
			f: impl Fn(T::CurrencyId, T::CurrencyId) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_get_order_details(f: impl Fn(T::OrderId) -> Option<T::OrderDetails> + 'static) {
			register_call!(f);
		}
	}

	impl<T: Config> TokenSwaps<T::AccountId> for Pallet<T> {
		type Balance = T::Balance;
		type CurrencyId = T::CurrencyId;
		type OrderDetails = T::OrderDetails;
		type OrderId = T::OrderId;
		type Ratio = T::Ratio;

		fn place_order(
			a: T::AccountId,
			b: Self::CurrencyId,
			c: Self::CurrencyId,
			d: Self::Balance,
			e: OrderRatio<Self::Ratio>,
		) -> Result<Self::OrderId, DispatchError> {
			execute_call!((a, b, c, d, e))
		}

		fn update_order(
			a: Self::OrderId,
			b: Self::Balance,
			c: OrderRatio<Self::Ratio>,
		) -> DispatchResult {
			execute_call!((a, b, c))
		}

		fn cancel_order(a: Self::OrderId) -> DispatchResult {
			execute_call!(a)
		}

		fn is_active(a: Self::OrderId) -> bool {
			execute_call!(a)
		}

		fn valid_pair(a: Self::CurrencyId, b: Self::CurrencyId) -> bool {
			execute_call!((a, b))
		}

		fn get_order_details(a: Self::OrderId) -> Option<Self::OrderDetails> {
			execute_call!(a)
		}
	}
}
