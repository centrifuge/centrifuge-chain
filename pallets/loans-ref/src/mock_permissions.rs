#[allow(dead_code)]
#[frame_support::pallet]
pub mod pallet_mock_permissions {
	use std::{cell::RefCell, collections::HashMap};

	use cfg_traits::Permissions;
	use cfg_types::permissions::{PermissionScope, Role};
	use frame_support::pallet_prelude::*;

	type CurrencyId = u32;
	type AccountId = u64;
	type Scope = PermissionScope<u64, CurrencyId>;
	type HasFn = Box<dyn Fn(Scope, AccountId, Role) -> bool>;

	thread_local! {
		static HAS_FNS: RefCell<HashMap<u64, HasFn>> = RefCell::new(HashMap::default());
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type IdCall<T: Config> = StorageValue<_, u64, OptionQuery>;

	impl<T: Config> Pallet<T> {
		pub fn expect_has(f: impl Fn(Scope, AccountId, Role) -> bool + 'static) {
			HAS_FNS.with(|state| {
				let mut registry = state.borrow_mut();
				let fn_id = registry.len() as u64;
				registry.insert(fn_id, Box::new(f));
				IdCall::<T>::put(fn_id);
			})
		}
	}

	impl<T: Config> Permissions<AccountId> for Pallet<T> {
		type Error = DispatchError;
		type Ok = ();
		type Role = Role;
		type Scope = Scope;

		fn has(scope: Self::Scope, who: AccountId, role: Self::Role) -> bool {
			let fn_id = IdCall::<T>::get().expect("Must be an expectation for this call");

			HAS_FNS.with(|state| {
				let registry = state.borrow();
				let call = registry.get(&fn_id).expect("fn stored");
				call(scope, who, role)
			})
		}

		fn add(_: Self::Scope, _: AccountId, _: Self::Role) -> DispatchResult {
			unimplemented!()
		}

		fn remove(_: Self::Scope, _: AccountId, _: Self::Role) -> DispatchResult {
			unimplemented!()
		}
	}
}
