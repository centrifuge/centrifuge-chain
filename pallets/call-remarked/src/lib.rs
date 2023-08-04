#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_utility::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Remark: Parameter;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Stored data off chain.
		Remark { remark: T::Remark },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Index and store data off chain.
		#[pallet::call_index(0)]
		#[pallet::weight(10_000_000)]
		pub fn remark_call(
			origin: OriginFor<T>,
			call: <T as pallet_utility::Config>::RuntimeCall,
			remark: T::Remark,
		) -> DispatchResultWithPostInfo {
			let info = <pallet_utility::Pallet<T>>::batch_all(origin, vec![call]);

			Self::deposit_event(Event::Remark { remark });

			info
		}
	}
}
