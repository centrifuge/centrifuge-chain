use cfg_traits::PreConditions;
use frame_benchmarking::{v2::*, whitelisted_caller};
use frame_system::RawOrigin;

use crate::pallet::{Call, Collection, Config, Pallet};

#[cfg(test)]
fn init_mocks() {
	use crate::mock::{MockChangeGuard, MockIsAdmin, MockProvider};

	MockIsAdmin::mock_check(|_| true);
	MockChangeGuard::mock_note(|_, change| {
		MockChangeGuard::mock_released(move |_, _| Ok(change.clone()));
		Ok(Default::default())
	});
	MockProvider::mock_get(|_, _| Ok((Default::default(), Default::default())));
}

#[benchmarks(
    where
        T::CollectionId: Default,
        T::OracleKey: Default + From<u32>,
        T::Hash: Default,
    )]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn propose_update_feeders(n: Linear<1, 10>) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let admin: T::AccountId = whitelisted_caller();

		T::IsAdmin::satisfy((admin.clone(), T::CollectionId::default()));

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

		T::IsAdmin::satisfy((admin.clone(), T::CollectionId::default()));

		let feeders = (0..n)
			.map(|i| account("feeder", i, 0))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();

		Pallet::<T>::propose_update_feeders(
			RawOrigin::Signed(admin.clone()).into(),
			T::CollectionId::default(),
			T::OracleKey::default(),
			feeders,
		)?;

		#[extrinsic_call]
		apply_update_feeders(
			RawOrigin::Signed(admin),
			T::CollectionId::default(),
			T::Hash::default(),
		);

		Ok(())
	}

	#[benchmark]
	fn update_collection(n: Linear<1, 10>, m: Linear<1, 10>) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let admin: T::AccountId = whitelisted_caller();

		T::IsAdmin::satisfy((admin.clone(), T::CollectionId::default()));

		// m keys with n feeders
		for k in 0..m {
			let feeders = (0..n)
				.map(|i| account("feeder", i, 0))
				.collect::<Vec<_>>()
				.try_into()
				.unwrap();

			Pallet::<T>::propose_update_feeders(
				RawOrigin::Signed(admin.clone()).into(),
				T::CollectionId::default(),
				T::OracleKey::from(k),
				feeders,
			)?;

			Pallet::<T>::apply_update_feeders(
				RawOrigin::Signed(admin.clone()).into(),
				T::CollectionId::default(),
				T::Hash::default(),
			)?;
		}

		#[extrinsic_call]
		update_collection(RawOrigin::Signed(admin), Default::default());

		assert_eq!(
			Collection::<T>::get(T::CollectionId::default()).len() as u32,
			m
		);

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}
