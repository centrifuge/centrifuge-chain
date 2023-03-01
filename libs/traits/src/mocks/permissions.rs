pub use pallet_mock_permissions::*;

#[allow(dead_code)]
#[frame_support::pallet]
pub mod pallet_mock_permissions {
	use cfg_traits::Permissions;
	use cfg_types::permissions::Role;
	use frame_support::pallet_prelude::*;

	use super::super::builder::CallId;
	use crate::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Scope;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type CallIds<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<Blake2_128 as frame_support::StorageHasher>::Output,
		CallId,
	>;

	impl<T: Config> Pallet<T> {
		pub fn mock_has(f: impl Fn(T::Scope, T::AccountId, Role) -> bool + 'static) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_add(f: impl Fn(T::Scope, T::AccountId, Role) -> bool + 'static) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_remove(f: impl Fn(T::Scope, T::AccountId, Role) -> bool + 'static) {
			register_call!(move |(a, b, c)| f(a, b, c));
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
}
