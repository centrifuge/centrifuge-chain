pub use pallet_mock_pools::*;

#[allow(dead_code)]
#[frame_support::pallet]
mod pallet_mock_pools {
	use std::{cell::RefCell, collections::HashMap, thread::LocalKey};

	use cfg_primitives::Moment;
	use cfg_traits::{PoolInspect, PoolReserve, PriceValue};
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_arithmetic::FixedU128;

	type PoolId = u64;
	type TrancheId = u64;
	type Balance = u128;
	type CurrencyId = u32;
	type Rate = FixedU128;
	type AccountId = u64;

	type PoolExistsFn = Box<dyn Fn(PoolId) -> bool>;
	type AccountForFn = Box<dyn Fn(PoolId) -> AccountId>;
	type WithdrawFn = Box<dyn Fn((PoolId, AccountId, Balance)) -> DispatchResult>;
	type DepositFn = Box<dyn Fn((PoolId, AccountId, Balance)) -> DispatchResult>;

	type CallId = u64;
	type CallStorage<F> = LocalKey<RefCell<(CallType, HashMap<u64, Box<F>>)>>;

	#[derive(Clone, Copy, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub enum CallType {
		PoolExists,
		AccountFor,
		Withdraw,
		Deposit,
	}

	thread_local! {
		static POOL_EXISTS_FNS: RefCell<(CallType, HashMap<CallId, PoolExistsFn>)>
			= RefCell::new((CallType::PoolExists, HashMap::default()));
		static ACCOUNT_FOR_FNS: RefCell<(CallType, HashMap<CallId, AccountForFn>)>
			= RefCell::new((CallType::PoolExists, HashMap::default()));
		static WITHDRAW_FNS: RefCell<(CallType, HashMap<CallId, WithdrawFn>)>
			= RefCell::new((CallType::PoolExists, HashMap::default()));
		static DEPOSIT_FNS: RefCell<(CallType, HashMap<CallId, DepositFn>)>
			= RefCell::new((CallType::PoolExists, HashMap::default()));
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type IdCall<T: Config> = StorageMap<_, Blake2_128Concat, CallType, CallId>;

	impl<T: Config> Pallet<T> {
		fn register_call<F: Fn(Args) -> R + 'static, Args, R>(
			call_storage: &'static CallStorage<dyn Fn(Args) -> R>,
			f: F,
		) {
			call_storage.with(|state| {
				let (call_type, registry) = &mut *state.borrow_mut();
				let fn_id = registry.len() as u64;
				registry.insert(fn_id, Box::new(f));
				IdCall::<T>::insert(call_type, fn_id);
			});
		}

		fn execute_call<Args, R>(
			call_storage: &'static CallStorage<dyn Fn(Args) -> R>,
			args: Args,
		) -> R {
			call_storage.with(|state| {
				let (call_type, registry) = &*state.borrow();
				let fn_id =
					IdCall::<T>::get(call_type).expect("Must be an expectation for this call");
				let call = registry.get(&fn_id).unwrap();
				call(args)
			})
		}

		pub fn pool_exists_for(f: impl Fn(PoolId) -> bool + 'static) {
			Self::register_call(&POOL_EXISTS_FNS, f);
		}

		pub fn expect_account_for(f: impl Fn(PoolId) -> AccountId + 'static) {
			Self::register_call(&ACCOUNT_FOR_FNS, f);
		}

		pub fn expect_withdraw(f: impl Fn(PoolId, AccountId, Balance) -> DispatchResult + 'static) {
			Self::register_call(&WITHDRAW_FNS, move |(a, b, c)| f(a, b, c));
		}

		pub fn expect_deposit(f: impl Fn(PoolId, AccountId, Balance) -> DispatchResult + 'static) {
			Self::register_call(&DEPOSIT_FNS, move |(a, b, c)| f(a, b, c));
		}
	}

	impl<T: Config> PoolInspect<AccountId, CurrencyId> for Pallet<T> {
		type Moment = Moment;
		type PoolId = PoolId;
		type Rate = Rate;
		type TrancheId = TrancheId;

		fn pool_exists(pool_id: PoolId) -> bool {
			Self::execute_call(&POOL_EXISTS_FNS, pool_id)
		}

		fn tranche_exists(_: PoolId, _: TrancheId) -> bool {
			unimplemented!()
		}

		fn get_tranche_token_price(
			_: PoolId,
			_: TrancheId,
		) -> Option<PriceValue<CurrencyId, Rate, Moment>> {
			unimplemented!()
		}

		fn account_for(pool_id: PoolId) -> AccountId {
			Self::execute_call(&ACCOUNT_FOR_FNS, pool_id)
		}
	}

	impl<T: Config> PoolReserve<AccountId, CurrencyId> for Pallet<T> {
		type Balance = Balance;

		fn withdraw(pool_id: PoolId, to: AccountId, amount: Balance) -> DispatchResult {
			Self::execute_call(&WITHDRAW_FNS, (pool_id, to, amount))
		}

		fn deposit(pool_id: PoolId, from: AccountId, amount: Balance) -> DispatchResult {
			Self::execute_call(&DEPOSIT_FNS, (pool_id, from, amount))
		}
	}
}
