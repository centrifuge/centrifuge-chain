#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::fees::PayFee;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_pay(f: impl Fn(&T::AccountId) -> DispatchResult + 'static) {
			register_call!(f);
		}
	}

	impl<T: Config> PayFee<T::AccountId> for Pallet<T> {
		fn pay(a: &T::AccountId) -> DispatchResult {
			execute_call!(a)
		}
	}
}
