use super::*;
use crate::loan_type::BulletLoan;
use crate::test_utils::initialise_test_pool;
use crate::types::WriteOffGroup;
use crate::{Config as LoanConfig, Event as LoanEvent, Pallet as LoanPallet};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::{Currency, UnfilteredDispatchable};
use frame_system::RawOrigin;
use pallet_balances::Pallet as BalancePallet;
use pallet_pool::CurrencyIdOf;
use runtime_common::{Amount, Rate, CFG};
use test_utils::{
	assert_last_event, create_nft_class, create_pool, expect_asset_owner, mint_nft,
	GetUSDCurrencyId,
};

pub struct Pallet<T: Config>(LoanPallet<T>);

pub trait Config:
	LoanConfig<ClassId = <Self as pallet_uniques::Config>::ClassId>
	+ pallet_balances::Config
	+ pallet_uniques::Config
	+ pallet_pool::Config
{
}

fn make_free_balance_minimum<T>(account: T::AccountId)
where
	T: Config + pallet_balances::Config,
	<T as pallet_balances::Config>::Balance: From<u128>,
{
	let min_balance: T::Balance = (10u128 * CFG).into();
	let _ = BalancePallet::<T>::make_free_balance_be(&account, min_balance);
}

fn whitelist_acc<T: frame_system::Config>(acc: &T::AccountId) {
	frame_benchmarking::benchmarking::add_to_whitelist(
		frame_system::Account::<T>::hashed_key_for(acc).into(),
	);
}

fn create_and_init_pool<T: Config>() -> (
	T::AccountId,
	PoolIdOf<T>,
	T::AccountId,
	<T as LoanConfig>::ClassId,
)
where
	<T as pallet_balances::Config>::Balance: From<u128>,
	CurrencyIdOf<T>: From<u32>,
	PoolIdOf<T>: From<<T as pallet_pool::Config>::PoolId>,
	<T as pallet_uniques::Config>::ClassId: From<u64>,
{
	// create pool
	let pool_owner = account::<T::AccountId>("owner", 0, 0);
	make_free_balance_minimum::<T>(pool_owner.clone());
	let pool_id: PoolIdOf<T> = create_pool::<T, GetUSDCurrencyId>(pool_owner.clone()).into();

	// initialise pool on loan
	let loan_account = LoanPallet::<T>::account_id();
	make_free_balance_minimum::<T>(loan_account.clone());
	let loan_class_id = initialise_test_pool::<T>(
		pool_id,
		1,
		T::AdminOrigin::successful_origin(),
		pool_owner.clone(),
		Some(loan_account.clone()),
	);

	whitelist_acc::<T>(&pool_owner);
	whitelist_acc::<T>(&loan_account);
	(pool_owner, pool_id, loan_account, loan_class_id)
}

fn create_asset<T: Config>() -> (T::AccountId, AssetOf<T>)
where
	<T as pallet_balances::Config>::Balance: From<u128>,
	<T as pallet_uniques::Config>::ClassId: From<u64>,
{
	// create asset
	let loan_owner = account::<T::AccountId>("caller", 0, 0);
	make_free_balance_minimum::<T>(loan_owner.clone());
	let asset_class_id = create_nft_class::<T>(2, loan_owner.clone(), None);
	let asset_instance_id = mint_nft::<T>(loan_owner.clone(), asset_class_id);
	let asset = Asset(asset_class_id, asset_instance_id);
	whitelist_acc::<T>(&loan_owner);
	(loan_owner, asset)
}

benchmarks! {
	where_clause {
		where
		<T as pallet_uniques::Config>::ClassId: From<u64>,
		<T as pallet_balances::Config>::Balance: From<u128>,
		CurrencyIdOf<T>: From<u32>,
		<T as LoanConfig>::Rate: From<Rate>,
		<T as LoanConfig>::Amount: From<Amount>,
		PoolIdOf<T>: From<<T as pallet_pool::Config>::PoolId> }

	initialise_pool{
		let origin = T::AdminOrigin::successful_origin();
		let pool_id: PoolIdOf<T> = Default::default();
		let class_id: <T as LoanConfig>::ClassId = Default::default();
		let call = Call::<T>::initialise_pool(pool_id, class_id);
	}:{ call.dispatch_bypass_filter(origin)? }
	verify{
		let got_class_id = PoolToLoanNftClass::<T>::get(pool_id).expect("pool must be initialised");
		assert_eq!(class_id, got_class_id);
		let got_pool_id = LoanNftClassToPool::<T>::get(got_class_id).expect("nft class id must be initialised");
		assert_eq!(pool_id, got_pool_id);
	}

	issue_loan{
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, asset)
	verify{
		// assert loan issue event
		let loan_id: T::LoanId = 1u128.into();
		assert_last_event::<T>(LoanEvent::LoanIssued(pool_id, loan_id, asset).into());

		// asset owner must be loan account
		expect_asset_owner::<T>(asset, loan_account);

		// loan owner must be caller
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_owner::<T>(loan_asset, loan_owner);
	}

	activate_loan{
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_type = LoanType::BulletLoan(BulletLoan::new(
			// advance rate 80%
			Rate::saturating_from_rational(80, 100).into(),
			// expected loss over asset maturity 0.15%
			Rate::saturating_from_rational(15, 10000).into(),
			// collateral value
			Amount::from_inner(125 * CFG).into(),
			// 4%
			math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap().into(),
			// 2 years
			math::seconds_per_year() * 2,
		));
		// interest rate is 5%
		let rp: T::Rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap().into();
		let origin = T::AdminOrigin::successful_origin();
		let loan_id: T::LoanId = 1u128.into();
		let call = Call::<T>::activate_loan(pool_id, loan_id, rp, loan_type);
	}:{ call.dispatch_bypass_filter(origin)? }
	verify{
		assert_last_event::<T>(LoanEvent::LoanActivated(pool_id, loan_id).into());
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.loan_type, loan_type);
		assert_eq!(loan_info.status, LoanStatus::Active);
		assert_eq!(loan_info.rate_per_sec, rp);
	}

	add_write_off_group_to_pool{
		let origin = T::AdminOrigin::successful_origin();
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>();
		let write_off_group = WriteOffGroup {
			// 10%
			percentage: Rate::saturating_from_rational(10, 100).into(),
			overdue_days: 3
		};
		let call = Call::<T>::add_write_off_group_to_pool(pool_id, write_off_group);
	}:{ call.dispatch_bypass_filter(origin)? }
	verify{
		let index = 0u32;
		assert_last_event::<T>(LoanEvent::WriteOffGroupAdded(pool_id, index).into());
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::MockRuntime,
);
