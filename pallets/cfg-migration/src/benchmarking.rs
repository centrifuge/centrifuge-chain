use frame_benchmarking::{benchmarks, whitelisted_caller};
use sp_core::H160;

benchmarks! {
	migrate {
		let caller: T::AccountId = whitelisted_caller();
		let receiver: H160 = H160::repeat_byte(0x42);
		let amount: BalanceOf<T> = 100u32.into();

		// Fund the caller
		T::Currency::make_free_balance_be(&caller, amount);
	}: _(RawOrigin::Signed(caller), amount, receiver)
}
