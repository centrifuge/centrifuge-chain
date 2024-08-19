#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::investments::ForeignInvestmentHooks;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call, CallHandler};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Amount;
		type TrancheAmount;
		type CurrencyId;
		type InvestmentId;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_fulfill_cancel_investment(
			f: impl Fn(
					&T::AccountId,
					T::InvestmentId,
					T::CurrencyId,
					T::Amount,
					T::Amount,
				) -> DispatchResult
				+ 'static,
		) -> CallHandler {
			register_call!(move |(a, b, c, d, e)| f(a, b, c, d, e))
		}

		pub fn mock_fulfill_collect_investment(
			f: impl Fn(
					&T::AccountId,
					T::InvestmentId,
					T::CurrencyId,
					T::Amount,
					T::TrancheAmount,
				) -> DispatchResult
				+ 'static,
		) -> CallHandler {
			register_call!(move |(a, b, c, d, e)| f(a, b, c, d, e))
		}

		pub fn mock_fulfill_collect_redemption(
			f: impl Fn(
					&T::AccountId,
					T::InvestmentId,
					T::CurrencyId,
					T::TrancheAmount,
					T::Amount,
				) -> DispatchResult
				+ 'static,
		) -> CallHandler {
			register_call!(move |(a, b, c, d, e)| f(a, b, c, d, e))
		}
	}

	impl<T: Config> ForeignInvestmentHooks<T::AccountId> for Pallet<T> {
		type Amount = T::Amount;
		type CurrencyId = T::CurrencyId;
		type InvestmentId = T::InvestmentId;
		type TrancheAmount = T::TrancheAmount;

		/// An async cancellation has been done
		fn fulfill_cancel_investment(
			a: &T::AccountId,
			b: Self::InvestmentId,
			c: Self::CurrencyId,
			d: Self::Amount,
			e: Self::Amount,
		) -> DispatchResult {
			execute_call!((a, b, c, d, e))
		}

		/// An async investment collection has been done
		fn fulfill_collect_investment(
			a: &T::AccountId,
			b: Self::InvestmentId,
			c: Self::CurrencyId,
			d: Self::Amount,
			e: Self::TrancheAmount,
		) -> DispatchResult {
			execute_call!((a, b, c, d, e))
		}

		/// An async redemption collection has been done
		fn fulfill_collect_redemption(
			a: &T::AccountId,
			b: Self::InvestmentId,
			c: Self::CurrencyId,
			d: Self::TrancheAmount,
			e: Self::Amount,
		) -> DispatchResult {
			execute_call!((a, b, c, d, e))
		}
	}
}
