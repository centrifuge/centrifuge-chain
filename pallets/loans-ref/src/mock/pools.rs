pub use pallet_mock_pools::*;

#[allow(dead_code)]
#[frame_support::pallet]
mod pallet_mock_pools {
	use std::{cell::RefCell, collections::HashMap};

	use cfg_primitives::Moment;
	use cfg_traits::{PoolInspect, PoolReserve, PriceValue};
	use frame_support::pallet_prelude::*;
	use sp_arithmetic::FixedU128;

	type PoolId = u64;
	type TrancheId = u64;
	type Balance = u128;
	type CurrencyId = u32;
	type Rate = FixedU128;
	type AccountId = u64;

	type PoolExistsFn = Box<dyn Fn(PoolId) -> bool>;
	type AccountForFn = Box<dyn Fn(PoolId) -> AccountId>;
	type WithdrawFn = Box<dyn Fn(PoolId, AccountId, Balance) -> DispatchResult>;
	type DepositFn = Box<dyn Fn(PoolId, AccountId, Balance) -> DispatchResult>;

	thread_local! {
		static POOL_EXISTS_FNS: RefCell<HashMap<u64, PoolExistsFn>> = RefCell::new(HashMap::default());
		static ACCOUNT_FOR_FNS: RefCell<HashMap<u64, AccountForFn>> = RefCell::new(HashMap::default());
		static WITHDRAW_FNS: RefCell<HashMap<u64, WithdrawFn>> = RefCell::new(HashMap::default());
		static DEPOSIT_FNS: RefCell<HashMap<u64, DepositFn>> = RefCell::new(HashMap::default());
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type IdCall<T: Config> = StorageValue<_, u64, OptionQuery>;

	impl<T: Config> Pallet<T> {
		pub fn pool_exists_for(f: impl Fn(PoolId) -> bool + 'static) {
			POOL_EXISTS_FNS.with(|state| {
				let mut registry = state.borrow_mut();
				let fn_id = registry.len() as u64;
				registry.insert(fn_id, Box::new(f));
				IdCall::<T>::put(fn_id);
			})
		}

		pub fn expect_account_for(f: impl Fn(PoolId) -> AccountId + 'static) {
			ACCOUNT_FOR_FNS.with(|state| {
				let mut registry = state.borrow_mut();
				let fn_id = registry.len() as u64;
				registry.insert(fn_id, Box::new(f));
				IdCall::<T>::put(fn_id);
			})
		}

		pub fn expect_withdraw(f: impl Fn(PoolId, AccountId, Balance) -> DispatchResult + 'static) {
			WITHDRAW_FNS.with(|state| {
				let mut registry = state.borrow_mut();
				let fn_id = registry.len() as u64;
				registry.insert(fn_id, Box::new(f));
				IdCall::<T>::put(fn_id);
			})
		}

		pub fn expect_deposit(f: impl Fn(PoolId, AccountId, Balance) -> DispatchResult + 'static) {
			DEPOSIT_FNS.with(|state| {
				let mut registry = state.borrow_mut();
				let fn_id = registry.len() as u64;
				registry.insert(fn_id, Box::new(f));
				IdCall::<T>::put(fn_id);
			})
		}
	}

	impl<T: Config> PoolInspect<AccountId, CurrencyId> for Pallet<T> {
		type Moment = Moment;
		type PoolId = PoolId;
		type Rate = Rate;
		type TrancheId = TrancheId;

		fn pool_exists(pool_id: PoolId) -> bool {
			let fn_id = IdCall::<T>::get().expect("Must be an expectation for this call");

			POOL_EXISTS_FNS.with(|state| {
				let registry = state.borrow();
				let call = registry.get(&fn_id).expect("fn stored");
				call(pool_id)
			})
		}

		fn tranche_exists(_: PoolId, _: TrancheId) -> bool {
			unimplemented!()
		}

		fn get_tranche_token_price(
			_: Self::PoolId,
			_: Self::TrancheId,
		) -> Option<PriceValue<CurrencyId, Rate, Moment>> {
			unimplemented!()
		}

		fn account_for(pool_id: Self::PoolId) -> AccountId {
			let fn_id = IdCall::<T>::get().expect("Must be an expectation for this call");

			ACCOUNT_FOR_FNS.with(|state| {
				let registry = state.borrow();
				let call = registry.get(&fn_id).expect("fn stored");
				call(pool_id)
			})
		}
	}

	impl<T: Config> PoolReserve<AccountId, CurrencyId> for Pallet<T> {
		type Balance = Balance;

		fn withdraw(pool_id: PoolId, to: AccountId, amount: Balance) -> DispatchResult {
			let fn_id = IdCall::<T>::get().expect("Must be an expectation for this call");

			WITHDRAW_FNS.with(|state| {
				let registry = state.borrow();
				let call = registry.get(&fn_id).expect("fn stored");
				call(pool_id, to, amount)
			})
		}

		fn deposit(pool_id: PoolId, from: AccountId, amount: Balance) -> DispatchResult {
			let fn_id = IdCall::<T>::get().expect("Must be an expectation for this call");

			DEPOSIT_FNS.with(|state| {
				let registry = state.borrow();
				let call = registry.get(&fn_id).expect("fn stored");
				call(pool_id, from, amount)
			})
		}
	}
}
