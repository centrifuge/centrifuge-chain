#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::investments::ForeignInvestment;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

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
		pub fn mock_increase_foreign_investment(
			f: impl Fn(&T::AccountId, T::InvestmentId, T::Amount, T::CurrencyId) -> DispatchResult
				+ 'static,
		) {
			register_call!(move |(a, b, c, d)| f(a, b, c, d));
		}

		pub fn mock_cancel_foreign_investment(
			f: impl Fn(&T::AccountId, T::InvestmentId, T::CurrencyId) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_increase_foreign_redemption(
			f: impl Fn(
					&T::AccountId,
					T::InvestmentId,
					T::TrancheAmount,
					T::CurrencyId,
				) -> DispatchResult
				+ 'static,
		) {
			register_call!(move |(a, b, c, d)| f(a, b, c, d));
		}

		pub fn mock_cancel_foreign_redemption(
			f: impl Fn(
					&T::AccountId,
					T::InvestmentId,
					T::CurrencyId,
				) -> Result<T::TrancheAmount, DispatchError>
				+ 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}
	}

	impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
		type Amount = T::Amount;
		type CurrencyId = T::CurrencyId;
		type InvestmentId = T::InvestmentId;
		type TrancheAmount = T::TrancheAmount;

		fn increase_foreign_investment(
			a: &T::AccountId,
			b: Self::InvestmentId,
			c: Self::Amount,
			d: Self::CurrencyId,
		) -> DispatchResult {
			execute_call!((a, b, c, d))
		}

		fn cancel_foreign_investment(
			a: &T::AccountId,
			b: Self::InvestmentId,
			c: Self::CurrencyId,
		) -> DispatchResult {
			execute_call!((a, b, c))
		}

		fn increase_foreign_redemption(
			a: &T::AccountId,
			b: Self::InvestmentId,
			c: Self::TrancheAmount,
			d: Self::CurrencyId,
		) -> DispatchResult {
			execute_call!((a, b, c, d))
		}

		fn cancel_foreign_redemption(
			a: &T::AccountId,
			b: Self::InvestmentId,
			c: Self::CurrencyId,
		) -> Result<T::TrancheAmount, DispatchError> {
			execute_call!((a, b, c))
		}
	}
}
