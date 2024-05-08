#[frame_support::pallet(dev_mode)]
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
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

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

	impl<T: Config> frame_support::traits::UnixTime for Pallet<T>
	where
		T::Moment: Into<u64>,
	{
		fn now() -> std::time::Duration {
			core::time::Duration::from_millis(<Pallet<T> as Time>::now().into())
		}
	}
}
