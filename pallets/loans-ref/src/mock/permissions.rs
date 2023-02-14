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
		pub fn expect_has(f: impl Fn(T::Scope, T::AccountId, Role) -> bool + 'static) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}
	}

	impl<T: Config> Permissions<T::AccountId> for Pallet<T> {
		type Error = DispatchError;
		type Ok = ();
		type Role = Role;
		type Scope = T::Scope;

		fn has(scope: Self::Scope, who: T::AccountId, role: Self::Role) -> bool {
			execute_call!((scope, who, role))
		}

		fn add(_: Self::Scope, _: T::AccountId, _: Self::Role) -> DispatchResult {
			unimplemented!()
		}

		fn remove(_: Self::Scope, _: T::AccountId, _: Self::Role) -> DispatchResult {
			unimplemented!()
		}
	}
}
