use cfg_traits::{benchmarking::PoolBenchmarkHelper, changes::ChangeGuard, ValueProvider};
use frame_benchmarking::{v2::*, whitelisted_caller};
use frame_support::storage::bounded_vec::BoundedVec;
use frame_system::RawOrigin;

use crate::{
	pallet::{Call, Collection, Config, Pallet},
	types::Change,
};

#[cfg(test)]
fn init_mocks() {
	use crate::mock::{MockChangeGuard, MockIsAdmin, MockProvider};

	MockIsAdmin::mock_check(|_| true);
	MockProvider::mock_get(|_, _| Ok(Some((Default::default(), Default::default()))));
	MockChangeGuard::mock_note(|_, change| {
		MockChangeGuard::mock_released(move |_, _| Ok(change.clone()));
		Ok(Default::default())
	});
}

mod util {
	use super::*;

	pub fn last_change_id_for<T>(
		key: T::OracleKey,
		feeders: &BoundedVec<T::AccountId, T::MaxFeedersPerKey>,
	) -> T::Hash
	where
		T: Config,
		T::CollectionId: Default,
	{
		// Emulate to note a change to later apply it
		T::ChangeGuard::note(
			T::CollectionId::default(),
			Change::<T>::Feeders(key, feeders.clone()).into(),
		)
		.unwrap()
	}
}

#[benchmarks(
    where
        T::CollectionId: Default,
        T::OracleKey: Default + From<u32>,
        T::OracleValue: Default,
        T::Timestamp: Default,
        T::Hash: Default,
        T::ChangeGuard: PoolBenchmarkHelper<PoolId = T::CollectionId, AccountId = T::AccountId>,
    )]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn propose_update_feeders(n: Linear<1, 10>) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let admin: T::AccountId = whitelisted_caller();

		T::ChangeGuard::bench_create_pool(T::CollectionId::default(), &admin);

		let feeders = (0..n)
			.map(|i| account("feeder", i, 0))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();

		#[extrinsic_call]
		propose_update_feeders(
			RawOrigin::Signed(admin),
			T::CollectionId::default(),
			T::OracleKey::default(),
			feeders,
		);

		Ok(())
	}

	#[benchmark]
	fn apply_update_feeders(n: Linear<1, 10>) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let admin: T::AccountId = whitelisted_caller();

		T::ChangeGuard::bench_create_pool(T::CollectionId::default(), &admin);

		let feeders: BoundedVec<_, _> = (0..n)
			.map(|i| account("feeder", i, 0))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();

		#[extrinsic_call]
		apply_update_feeders(
			RawOrigin::Signed(admin),
			T::CollectionId::default(),
			util::last_change_id_for::<T>(T::OracleKey::default(), &feeders),
		);

		Ok(())
	}

	#[benchmark]
	fn update_collection(n: Linear<1, 10>, m: Linear<1, 10>) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let admin: T::AccountId = whitelisted_caller();

		T::ChangeGuard::bench_create_pool(T::CollectionId::default(), &admin);

		let feeders: BoundedVec<T::AccountId, _> = (0..n)
			.map(|i| account("feeder", i, 0))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();

		// n feeders using m keys
		for k in 0..m {
			let key = T::OracleKey::from(k);

			for feeder in feeders.iter() {
				T::OracleProvider::set(
					&(feeder.clone(), T::CollectionId::default()),
					&key,
					Default::default(),
				);
			}

			Pallet::<T>::apply_update_feeders(
				RawOrigin::Signed(admin.clone()).into(),
				T::CollectionId::default(),
				util::last_change_id_for::<T>(key, &feeders),
			)?;
		}

		#[extrinsic_call]
		update_collection(RawOrigin::Signed(admin), T::CollectionId::default());

		assert_eq!(
			Collection::<T>::get(T::CollectionId::default()).len() as u32,
			m
		);

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}
