pub use pallet_mock_pools::*;

#[allow(dead_code)]
#[frame_support::pallet]
mod pallet_mock_pools {
	use std::{any::Any, cell::RefCell, collections::HashMap};

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

	type CallId = u64;

	#[derive(Clone, Copy, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub enum CallType {
		PoolExists,
		AccountFor,
		Withdraw,
		Deposit,
	}

	struct FnWrapper<Args, R>(Box<dyn Fn(Args) -> R>);

	trait Callable {
		fn as_any(&self) -> &dyn Any;
	}

	impl<Args: 'static, R: 'static> Callable for FnWrapper<Args, R> {
		fn as_any(&self) -> &dyn Any {
			self
		}
	}

	thread_local! {
		static CALLS: RefCell<HashMap<CallId, Box<dyn Callable>>>
			= RefCell::new(HashMap::default());
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type IdCall<T: Config> = StorageMap<_, Blake2_128Concat, CallType, CallId>;

	impl<T: Config> Pallet<T> {
		fn register_call<F: Fn(Args) -> R + 'static, Args: 'static, R: 'static>(
			call_type: CallType,
			f: F,
		) {
			CALLS.with(|state| {
				let registry = &mut *state.borrow_mut();
				let fn_id = registry.len() as u64;
				registry.insert(fn_id, Box::new(FnWrapper(Box::new(f))));
				IdCall::<T>::insert(call_type, fn_id);
			});
		}

		fn execute_call<Args: 'static, R: 'static>(call_type: CallType, args: Args) -> R {
			CALLS.with(|state| {
				let registry = &*state.borrow();
				let fn_id =
					IdCall::<T>::get(call_type).expect("Must be an expectation for this call");
				let call = registry.get(&fn_id).unwrap();
				call.as_any()
					.downcast_ref::<FnWrapper<Args, R>>()
					.unwrap()
					.0(args)
			})
		}

		pub fn pool_exists_for(f: impl Fn(PoolId) -> bool + 'static) {
			Self::register_call(CallType::PoolExists, f);
		}

		pub fn expect_account_for(f: impl Fn(PoolId) -> AccountId + 'static) {
			Self::register_call(CallType::AccountFor, f);
		}

		pub fn expect_withdraw(f: impl Fn(PoolId, AccountId, Balance) -> DispatchResult + 'static) {
			Self::register_call(CallType::Withdraw, move |(a, b, c)| f(a, b, c));
		}

		pub fn expect_deposit(f: impl Fn(PoolId, AccountId, Balance) -> DispatchResult + 'static) {
			Self::register_call(CallType::Deposit, move |(a, b, c)| f(a, b, c));
		}
	}

	impl<T: Config> PoolInspect<AccountId, CurrencyId> for Pallet<T> {
		type Moment = Moment;
		type PoolId = PoolId;
		type Rate = Rate;
		type TrancheId = TrancheId;

		fn pool_exists(pool_id: PoolId) -> bool {
			Self::execute_call(CallType::PoolExists, pool_id)
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
			Self::execute_call(CallType::AccountFor, pool_id)
		}
	}

	impl<T: Config> PoolReserve<AccountId, CurrencyId> for Pallet<T> {
		type Balance = Balance;

		fn withdraw(pool_id: PoolId, to: AccountId, amount: Balance) -> DispatchResult {
			Self::execute_call(CallType::Withdraw, (pool_id, to, amount))
		}

		fn deposit(pool_id: PoolId, from: AccountId, amount: Balance) -> DispatchResult {
			Self::execute_call(CallType::Deposit, (pool_id, from, amount))
		}
	}
}
