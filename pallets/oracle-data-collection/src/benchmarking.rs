use cfg_traits::{benchmarking::PoolBenchmarkHelper, changes::ChangeGuard, ValueProvider};
use frame_benchmarking::{v2::*, whitelisted_caller};
use frame_system::RawOrigin;

use crate::{
	pallet::{Call, Collection, Config, Pallet},
	types::Change,
};

#[cfg(test)]
fn init_mocks() {
	use crate::mock::{MockChangeGuard, MockIsAdmin, MockProvider, MockTime};

	MockIsAdmin::mock_check(|_| true);
	MockProvider::mock_get(|_, _| Ok(Some((Default::default(), Default::default()))));
	MockChangeGuard::mock_note(|_, change| {
		MockChangeGuard::mock_released(move |_, _| Ok(change.clone()));
		Ok(Default::default())
	});
	MockTime::mock_now(|| 0);
}

mod util {
	use super::*;

	pub fn last_change_id_for<T>(
		key: T::OracleKey,
		feeders: impl IntoIterator<Item = T::FeederId>,
	) -> T::Hash
	where
		T: Config,
		T::CollectionId: Default,
	{
		let feeders = crate::util::feeders_from(feeders).unwrap();

		// Emulate to note a change to later apply it
		T::ChangeGuard::note(
			T::CollectionId::default(),
			Change::<T>::Feeders(key, feeders).into(),
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
        T::FeederId: From<u32>,
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

		let feeders = crate::util::feeders_from((0..n).map(Into::into)).unwrap();

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

		let feeder_ids = (0..n).map(Into::into);
		let feeders = crate::util::feeders_from::<_, T::MaxFeedersPerKey>(feeder_ids).unwrap();

		let change_id = util::last_change_id_for::<T>(T::OracleKey::default(), feeders);

		#[extrinsic_call]
		apply_update_feeders(
			RawOrigin::Signed(admin),
			T::CollectionId::default(),
			change_id,
		);

		Ok(())
	}

	#[benchmark]
	fn update_collection(n: Linear<1, 10>, m: Linear<1, 10>) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let admin: T::AccountId = whitelisted_caller();

		T::ChangeGuard::bench_create_pool(T::CollectionId::default(), &admin);

		let feeder_ids = (0..n).map(Into::<T::FeederId>::into);
		let feeders = crate::util::feeders_from::<_, T::MaxFeedersPerKey>(feeder_ids).unwrap();

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
				util::last_change_id_for::<T>(key, feeders.clone()),
			)?;
		}

		// Worst case expect to read the max age
		Pallet::<T>::set_collection_max_age(
			RawOrigin::Signed(admin.clone()).into(),
			T::CollectionId::default(),
			T::Timestamp::default(),
		)
		.unwrap();

		#[extrinsic_call]
		update_collection(RawOrigin::Signed(admin), T::CollectionId::default());

		assert_eq!(
			Collection::<T>::get(T::CollectionId::default())
				.content
				.len() as u32,
			m
		);

		Ok(())
	}

	#[benchmark]
	fn set_collection_max_age() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let admin: T::AccountId = whitelisted_caller();

		T::ChangeGuard::bench_create_pool(T::CollectionId::default(), &admin);

		#[extrinsic_call]
		set_collection_max_age(
			RawOrigin::Signed(admin),
			T::CollectionId::default(),
			T::Timestamp::default(),
		);

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}
