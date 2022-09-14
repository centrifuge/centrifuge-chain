use super::*;

pub mod fix_evict_date {
	use frame_support::{log, traits::Get, weights::Weight};

	use super::*;

	pub const HARDCODED_EVICTED_DATE: u32 = 19200;

	#[cfg(feature = "try-runtime")]
	use frame_support::ensure; // Not in prelude for try-runtime

	#[cfg(feature = "try-runtime")]
	pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
		ensure!(
			LatestEvictedDate::<T>::get() == None,
			"State already initialized"
		);
		Ok(())
	}

	pub fn migrate<T: Config>() -> Weight {
		if LatestEvictedDate::<T>::get().is_none() {
			LatestEvictedDate::<T>::put(HARDCODED_EVICTED_DATE);
			log::info!("pallet_anchors: fix evict date");
			return T::DbWeight::get().writes(1);
		}

		0
	}

	#[cfg(feature = "try-runtime")]
	pub fn post_migrate<T: Config>() -> Result<(), &'static str> {
		ensure!(
			LatestEvictedDate::<T>::get() == Some(HARDCODED_EVICTED_DATE),
			"State not initialized"
		);
		Ok(())
	}
}

#[cfg(test)]
#[cfg(feature = "try-runtime")]
mod test {
	use frame_support::assert_ok;

	use super::*;
	use crate::{
		mock::{new_test_ext, Origin, Test},
		{self as pallet_anchors},
	};

	#[test]
	fn evict_anchors_working_after_migration() {
		new_test_ext().execute_with(|| {
			// Check migration:
			assert_ok!(fix_evict_date::pre_migrate::<Test>());
			assert!(fix_evict_date::post_migrate::<Test>().is_err());

			fix_evict_date::migrate::<Test>();

			assert_ok!(fix_evict_date::post_migrate::<Test>());
			assert!(fix_evict_date::pre_migrate::<Test>().is_err());

			// Check correct evict behaviour after migration:
			let current_day = common::MILLISECS_PER_DAY
				* (fix_evict_date::HARDCODED_EVICTED_DATE as u64 + MAX_LOOP_IN_TX * 3);

			pallet_timestamp::Pallet::<Test>::set_timestamp(current_day);

			assert_ok!(pallet_anchors::Pallet::<Test>::evict_anchors(
				Origin::signed(1)
			));

			assert_eq!(
				LatestEvictedDate::<Test>::get(),
				Some(fix_evict_date::HARDCODED_EVICTED_DATE + MAX_LOOP_IN_TX as u32)
			);
		});
	}
}
