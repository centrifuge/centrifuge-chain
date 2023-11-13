#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::IdentityCurrencyConversion;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Balance;
		type CurrencyId;
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
		pub fn mock_stable_to_stable(
			f: impl Fn(T::CurrencyId, T::CurrencyId, T::Balance) -> Result<T::Balance, DispatchError>
				+ 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}
	}

	impl<T: Config> IdentityCurrencyConversion for Pallet<T> {
		type Balance = T::Balance;
		type Currency = T::CurrencyId;
		type Error = DispatchError;

		fn stable_to_stable(
			a: Self::Currency,
			b: Self::Currency,
			c: Self::Balance,
		) -> Result<Self::Balance, DispatchError> {
			execute_call!((a, b, c))
		}
	}
}
