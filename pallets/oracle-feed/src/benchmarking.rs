use frame_benchmarking::{v2::*, whitelisted_caller};
use frame_system::RawOrigin;

use crate::pallet::{Call, Config, Pallet};

#[benchmarks(
    where
        T::OracleKey: From<u32>,
        T::OracleValue: Default,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn feed_first() -> Result<(), BenchmarkError> {
		let feeder: T::AccountId = whitelisted_caller();

		#[extrinsic_call]
		feed(
			RawOrigin::Signed(feeder),
			1.into(),
			T::OracleValue::default(),
		);

		Ok(())
	}

	#[benchmark]
	fn feed_again() -> Result<(), BenchmarkError> {
		let feeder: T::AccountId = whitelisted_caller();

		Pallet::<T>::feed(
			RawOrigin::Signed(feeder.clone()).into(),
			1.into(),
			T::OracleValue::default(),
		)?;

		#[extrinsic_call]
		feed(
			RawOrigin::Signed(feeder),
			1.into(),
			T::OracleValue::default(),
		);

		Ok(())
	}
}
