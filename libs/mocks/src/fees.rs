#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::fees::{Fee, FeeKey, Fees};
	use frame_support::{pallet_prelude::*, traits::tokens::Balance};
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Balance: Balance;
		type FeeKey: FeeKey;
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
		pub fn mock_fee_value(f: impl Fn(T::FeeKey) -> T::Balance + 'static) {
			register_call!(f);
		}

		pub fn mock_fee_to_author(
			f: impl Fn(&T::AccountId, Fee<T::Balance, T::FeeKey>) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_fee_to_burn(
			f: impl Fn(&T::AccountId, Fee<T::Balance, T::FeeKey>) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_fee_to_treasury(
			f: impl Fn(&T::AccountId, Fee<T::Balance, T::FeeKey>) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config> Fees for Pallet<T> {
		type AccountId = T::AccountId;
		type Balance = T::Balance;
		type FeeKey = T::FeeKey;

		fn fee_value(a: Self::FeeKey) -> Self::Balance {
			execute_call!(a)
		}

		fn fee_to_author(
			a: &Self::AccountId,
			b: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult {
			execute_call!((a, b))
		}

		fn fee_to_burn(a: &Self::AccountId, b: Fee<Self::Balance, Self::FeeKey>) -> DispatchResult {
			execute_call!((a, b))
		}

		fn fee_to_treasury(
			a: &Self::AccountId,
			b: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult {
			execute_call!((a, b))
		}
	}
}
