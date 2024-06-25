use cfg_traits::fees::PayFee;
use frame_benchmarking::{v2::*, whitelisted_caller};
use frame_support::traits::OriginTrait;
use frame_system::RawOrigin;
use parity_scale_codec::Decode;

use crate::pallet::{Call, Config, Pallet};

#[cfg(test)]
fn init_mocks() {
	crate::mock::MockTime::mock_now(|| 0);
	crate::mock::MockPayFee::mock_pay(|_| Ok(()));
}

#[benchmarks(
    where
        T::OracleKey: Default,
        T::OracleValue: Default,
        <T::RuntimeOrigin as OriginTrait>::AccountId: Clone + Decode,
        T::RuntimeOrigin: From<RawOrigin<<T::RuntimeOrigin as OriginTrait>::AccountId>>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn feed_with_fee() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let feeder: <T::RuntimeOrigin as OriginTrait>::AccountId = whitelisted_caller();

		T::FirstValuePayFee::add_pay_requirements(&feeder);

		#[extrinsic_call]
		feed(
			RawOrigin::Signed(feeder),
			T::OracleKey::default(),
			T::OracleValue::default(),
		);

		Ok(())
	}

	#[benchmark]
	fn feed_without_fee() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let feeder: <T::RuntimeOrigin as OriginTrait>::AccountId = whitelisted_caller();

		T::FirstValuePayFee::add_pay_requirements(&feeder);

		Pallet::<T>::feed(
			RawOrigin::Signed(feeder.clone()).into(),
			T::OracleKey::default(),
			T::OracleValue::default(),
		)?;

		#[extrinsic_call]
		feed(
			RawOrigin::Signed(feeder),
			T::OracleKey::default(),
			T::OracleValue::default(),
		);

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}
