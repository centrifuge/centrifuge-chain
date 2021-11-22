use super::*;
use crate::test_utils::initialise_test_pool;
use crate::{Config as LoanConfig, Event as LoanEvent, Pallet as LoanPallet};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, whitelist_account};
use frame_support::traits::{Currency, UnfilteredDispatchable};
use frame_system::RawOrigin;
use pallet_balances::Pallet as BalancePallet;
use pallet_pool::CurrencyIdOf;
use runtime_common::CFG;
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

benchmarks! {
	where_clause {
		where
		<T as pallet_uniques::Config>::ClassId: From<u64>,
		<T as pallet_balances::Config>::Balance: From<u128>,
		CurrencyIdOf<T>: From<u32>,
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
		// create pool
		let pool_owner = account::<T::AccountId>("owner", 0, 0);
		make_free_balance_minimum::<T>(pool_owner.clone());
		let pool_id: PoolIdOf<T> = create_pool::<T, GetUSDCurrencyId>(pool_owner.clone()).into();

		// initialise pool on loan
		let loan_account = LoanPallet::<T>::account_id();
		make_free_balance_minimum::<T>(loan_account.clone());
		let loan_class_id = initialise_test_pool::<T>(pool_id, 1, T::AdminOrigin::successful_origin(), pool_owner.clone(), Some(loan_account.clone()));

		// create asset
		let loan_owner = account::<T::AccountId>("caller", 0, 0);
		make_free_balance_minimum::<T>(loan_owner.clone());
		let asset_class_id = create_nft_class::<T>(2, loan_owner.clone(), None);
		let asset_instance_id = mint_nft::<T>(loan_owner.clone(), asset_class_id);
		let asset = Asset(asset_class_id, asset_instance_id);
		let caller = loan_owner.clone();

		// white list account
		whitelist_account!(pool_owner);
		whitelist_account!(loan_account);
		whitelist_account!(loan_owner);
	}:_(RawOrigin::Signed(caller.clone()), pool_id, asset)
	verify{
		// assert loan issue event
		let loan_id: T::LoanId = 1u128.into();
		assert_last_event::<T>(LoanEvent::LoanIssued(pool_id, loan_id, asset).into());

		// asset owner must be loan account
		expect_asset_owner::<T>(asset, LoanPallet::<T>::account_id());

		// loan owner must be caller
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_owner::<T>(loan_asset, caller);
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::MockRuntime,
);
