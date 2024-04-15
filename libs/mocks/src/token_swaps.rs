#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::swaps::{OrderInfo, OrderRatio, TokenSwaps};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type CurrencyId;
		type BalanceIn;
		type BalanceOut;
		type Ratio;
		type OrderId;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_place_order(
			f: impl Fn(
					T::AccountId,
					T::CurrencyId,
					T::CurrencyId,
					T::BalanceOut,
					OrderRatio<T::Ratio>,
				) -> Result<T::OrderId, DispatchError>
				+ 'static,
		) {
			register_call!(move |(a, b, c, d, e)| f(a, b, c, d, e));
		}

		pub fn mock_update_order(
			f: impl Fn(T::OrderId, T::BalanceOut, OrderRatio<T::Ratio>) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_cancel_order(f: impl Fn(T::OrderId) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_is_active(f: impl Fn(T::OrderId) -> bool + 'static) {
			register_call!(f);
		}

		pub fn mock_get_order_details(
			f: impl Fn(T::OrderId) -> Option<OrderInfo<T::BalanceOut, T::CurrencyId, T::Ratio>>
				+ 'static,
		) {
			register_call!(f);
		}

		pub fn mock_convert_by_market(
			f: impl Fn(
					T::CurrencyId,
					T::CurrencyId,
					T::BalanceOut,
				) -> Result<T::BalanceIn, DispatchError>
				+ 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_market_ratio(
			f: impl Fn(T::CurrencyId, T::CurrencyId) -> Result<T::Ratio, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_fill_order(
			f: impl Fn(T::AccountId, T::OrderId, T::BalanceOut) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c))
		}
	}

	impl<T: Config> TokenSwaps<T::AccountId> for Pallet<T> {
		type BalanceIn = T::BalanceIn;
		type BalanceOut = T::BalanceOut;
		type CurrencyId = T::CurrencyId;
		type OrderId = T::OrderId;
		type Ratio = T::Ratio;

		fn place_order(
			a: T::AccountId,
			b: Self::CurrencyId,
			c: Self::CurrencyId,
			d: Self::BalanceOut,
			e: OrderRatio<Self::Ratio>,
		) -> Result<Self::OrderId, DispatchError> {
			execute_call!((a, b, c, d, e))
		}

		fn update_order(
			a: Self::OrderId,
			b: Self::BalanceOut,
			c: OrderRatio<Self::Ratio>,
		) -> DispatchResult {
			execute_call!((a, b, c))
		}

		fn cancel_order(a: Self::OrderId) -> DispatchResult {
			execute_call!(a)
		}

		fn get_order_details(
			a: Self::OrderId,
		) -> Option<OrderInfo<Self::BalanceOut, Self::CurrencyId, Self::Ratio>> {
			execute_call!(a)
		}

		fn convert_by_market(
			a: Self::CurrencyId,
			b: Self::CurrencyId,
			c: Self::BalanceOut,
		) -> Result<Self::BalanceIn, DispatchError> {
			execute_call!((a, b, c))
		}

		fn market_ratio(
			a: Self::CurrencyId,
			b: Self::CurrencyId,
		) -> Result<Self::Ratio, DispatchError> {
			execute_call!((a, b))
		}

		fn fill_order(a: T::AccountId, b: Self::OrderId, c: Self::BalanceOut) -> DispatchResult {
			execute_call!((a, b, c))
		}
	}
}
