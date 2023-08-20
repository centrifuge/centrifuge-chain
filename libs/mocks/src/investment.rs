#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::investments::{Investment, InvestmentCollector};
	use cfg_types::investments::CollectedAmount;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Amount;
		type CurrencyId;
		type InvestmentId;
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
		pub fn mock_update_investment(
			f: impl Fn(&T::AccountId, T::InvestmentId, T::Amount) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_accepted_payment_currency(
			f: impl Fn(T::InvestmentId, T::CurrencyId) -> bool + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_investment(
			f: impl Fn(&T::AccountId, T::InvestmentId) -> Result<T::Amount, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_update_redemption(
			f: impl Fn(&T::AccountId, T::InvestmentId, T::Amount) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_accepted_payout_currency(
			f: impl Fn(T::InvestmentId, T::CurrencyId) -> bool + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_redemption(
			f: impl Fn(&T::AccountId, T::InvestmentId) -> Result<T::Amount, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_collect_investment(
			f: impl Fn(
					T::AccountId,
					T::InvestmentId,
				) -> Result<CollectedAmount<T::Amount>, DispatchError>
				+ 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_collect_redemption(
			f: impl Fn(
					T::AccountId,
					T::InvestmentId,
				) -> Result<CollectedAmount<T::Amount>, DispatchError>
				+ 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config> Investment<T::AccountId> for Pallet<T> {
		type Amount = T::Amount;
		type CurrencyId = T::CurrencyId;
		type Error = DispatchError;
		type InvestmentId = T::InvestmentId;

		fn update_investment(
			a: &T::AccountId,
			b: Self::InvestmentId,
			c: Self::Amount,
		) -> DispatchResult {
			execute_call!((a, b, c))
		}

		fn accepted_payment_currency(a: Self::InvestmentId, b: Self::CurrencyId) -> bool {
			execute_call!((a, b))
		}

		fn investment(
			a: &T::AccountId,
			b: Self::InvestmentId,
		) -> Result<Self::Amount, Self::Error> {
			execute_call!((a, b))
		}

		fn update_redemption(
			a: &T::AccountId,
			b: Self::InvestmentId,
			c: Self::Amount,
		) -> DispatchResult {
			execute_call!((a, b, c))
		}

		fn accepted_payout_currency(a: Self::InvestmentId, b: Self::CurrencyId) -> bool {
			execute_call!((a, b))
		}

		fn redemption(
			a: &T::AccountId,
			b: Self::InvestmentId,
		) -> Result<Self::Amount, Self::Error> {
			execute_call!((a, b))
		}
	}

	impl<T: Config> InvestmentCollector<T::AccountId> for Pallet<T> {
		type Error = DispatchError;
		type InvestmentId = T::InvestmentId;
		type Result = CollectedAmount<T::Amount>;

		fn collect_investment(
			a: T::AccountId,
			b: Self::InvestmentId,
		) -> Result<Self::Result, Self::Error> {
			execute_call!((a, b))
		}

		fn collect_redemption(
			a: T::AccountId,
			b: Self::InvestmentId,
		) -> Result<Self::Result, Self::Error> {
			execute_call!((a, b))
		}
	}
}
