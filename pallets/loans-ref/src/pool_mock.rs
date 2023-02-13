#[frame_support::pallet]
pub mod pallet_mock_pool {
	use std::{cell::RefCell, collections::HashMap};

	use cfg_primitives::Moment;
	use cfg_traits::{PoolInspect, PoolReserve, PriceValue};
	use frame_support::pallet_prelude::*;
	use sp_arithmetic::FixedU128;

	type PoolId = u64;
	type TrancheId = u64;
	type Balance = u64;
	type CurrencyId = u32;
	type Rate = FixedU128;
	type AccountId = u64;

	type WithdrawFn = Box<dyn Fn(PoolId, AccountId, Balance) -> DispatchResult>;

	thread_local! {
		pub static WITHDRAW_FNS: RefCell<HashMap<u64, WithdrawFn>> = RefCell::new(HashMap::default())
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type WithdrawIdCall<T: Config> = StorageValue<_, u64, OptionQuery>;

	impl<T: Config> Pallet<T> {
		pub fn expect_withdraw(f: impl Fn(PoolId, AccountId, Balance) -> DispatchResult + 'static) {
			WITHDRAW_FNS.with(|state| {
				let mut registry = state.borrow_mut();
				let fn_id = registry.len() as u64;
				registry.insert(fn_id, Box::new(f));
				WithdrawIdCall::<T>::put(fn_id);
			})
		}
	}

	impl<T: Config> PoolInspect<AccountId, CurrencyId> for Pallet<T> {
		type Moment = Moment;
		type PoolId = PoolId;
		type Rate = Rate;
		type TrancheId = TrancheId;

		fn pool_exists(pool_id: PoolId) -> bool {
			true
		}

		fn tranche_exists(pool_id: PoolId, tranche_id: TrancheId) -> bool {
			true
		}

		fn get_tranche_token_price(
			pool_id: Self::PoolId,
			tranche_id: Self::TrancheId,
		) -> Option<PriceValue<CurrencyId, Rate, Moment>> {
			todo!()
		}

		fn account_for(pool_id: Self::PoolId) -> AccountId {
			todo!()
		}
	}

	impl<T: Config> PoolReserve<AccountId, CurrencyId> for Pallet<T> {
		type Balance = Balance;

		fn withdraw(pool_id: PoolId, to: AccountId, amount: Balance) -> DispatchResult {
			let fn_id = WithdrawIdCall::<T>::get().expect("Must be an expectation for this call");

			WITHDRAW_FNS.with(|state| {
				let registry = state.borrow();
				let func = registry.get(&fn_id).expect("fn stored");
				func(pool_id, to, amount)
			})
		}

		fn deposit(pool_id: PoolId, from: AccountId, amount: Balance) -> DispatchResult {
			todo!()
		}
	}
}

mod mock {
	use cfg_traits::PoolReserve;
	use frame_support::{
		assert_ok,
		traits::{ConstU16, ConstU32, ConstU64},
	};
	use sp_core::H256;
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
	};

	use super::pallet_mock_pool;

	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
	type Block = frame_system::mocking::MockBlock<Test>;

	frame_support::construct_runtime!(
		pub enum Test where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system,
			MockPool: pallet_mock_pool,
		}
	);

	impl frame_system::Config for Test {
		type AccountData = ();
		type AccountId = u64;
		type BaseCallFilter = frame_support::traits::Everything;
		type BlockHashCount = ConstU64<250>;
		type BlockLength = ();
		type BlockNumber = u64;
		type BlockWeights = ();
		type DbWeight = ();
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Header = Header;
		type Index = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type MaxConsumers = ConstU32<16>;
		type OnKilledAccount = ();
		type OnNewAccount = ();
		type OnSetCode = ();
		type PalletInfo = PalletInfo;
		type RuntimeCall = RuntimeCall;
		type RuntimeEvent = RuntimeEvent;
		type RuntimeOrigin = RuntimeOrigin;
		type SS58Prefix = ConstU16<42>;
		type SystemWeightInfo = ();
		type Version = ();
	}

	impl pallet_mock_pool::Config for Test {}

	pub fn new_test_ext() -> sp_io::TestExternalities {
		let storage = frame_system::GenesisConfig::default()
			.build_storage::<Test>()
			.unwrap();

		sp_io::TestExternalities::new(storage)
	}

	#[test]
	fn wrong_test_example() {
		new_test_ext().execute_with(|| {
			MockPool::expect_withdraw(|_, _, amount| {
				assert_eq!(amount, 999);
				Ok(())
			});

			assert_ok!(MockPool::withdraw(1, 2, 1000));
		});
	}
}
