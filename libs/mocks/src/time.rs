#[frame_support::pallet]
pub mod pallet {
	use frame_support::{pallet_prelude::*, traits::Time};
	use mock_builder::{execute_call, register_call};
	use sp_runtime::traits::AtLeast32Bit;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Moment: AtLeast32Bit + Parameter + Default + Copy + MaxEncodedLen;
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
		pub fn mock_now(f: impl Fn() -> T::Moment + 'static) {
			register_call!(move |()| f());
		}
	}

	impl<T: Config> Time for Pallet<T> {
		type Moment = T::Moment;

		fn now() -> Self::Moment {
			execute_call!(())
		}
	}
}
