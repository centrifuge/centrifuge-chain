#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_primitives::TrancheId;
	use cfg_traits::Permissions;
	use cfg_types::permissions::{Role, TrancheInvestorInfo};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Scope;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_has(f: impl Fn(T::Scope, T::AccountId, Role) -> bool + 'static) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_add(f: impl Fn(T::Scope, T::AccountId, Role) -> DispatchResult + 'static) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_remove(f: impl Fn(T::Scope, T::AccountId, Role) -> DispatchResult + 'static) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_get(
			f: impl Fn(&(T::Scope, T::AccountId, TrancheId)) -> Option<TrancheInvestorInfo<TrancheId>>
				+ 'static,
		) {
			register_call!(f);
		}
	}

	impl<T: Config> Permissions<T::AccountId> for Pallet<T> {
		type Error = DispatchError;
		type Ok = ();
		type Role = Role;
		type Scope = T::Scope;

		fn has(a: Self::Scope, b: T::AccountId, c: Self::Role) -> bool {
			execute_call!((a, b, c))
		}

		fn add(a: Self::Scope, b: T::AccountId, c: Self::Role) -> DispatchResult {
			execute_call!((a, b, c))
		}

		fn remove(a: Self::Scope, b: T::AccountId, c: Self::Role) -> DispatchResult {
			execute_call!((a, b, c))
		}
	}

	impl<T: Config>
		orml_traits::GetByKey<
			(T::Scope, T::AccountId, TrancheId),
			Option<TrancheInvestorInfo<TrancheId>>,
		> for Pallet<T>
	{
		fn get(
			tuple: &(T::Scope, T::AccountId, TrancheId),
		) -> Option<TrancheInvestorInfo<TrancheId>> {
			execute_call!(tuple)
		}
	}
}
