use cfg_traits::swaps::SwapState;
use frame_support::derive_impl;
use sp_runtime::FixedU128;

use crate::pallet as pallet_swaps;

pub type AccountId = u64;
pub type Balance = u128;
pub type OrderId = u64;
pub type SwapId = u32;
pub type CurrencyId = u8;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		MockTokenSwaps: cfg_mocks::token_swaps::pallet,
		FulfilledSwapHook: cfg_mocks::status_notification::pallet,
		Swaps: pallet_swaps,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl cfg_mocks::token_swaps::pallet::Config for Runtime {
	type BalanceIn = Balance;
	type BalanceOut = Balance;
	type CurrencyId = CurrencyId;
	type OrderId = OrderId;
	type Ratio = FixedU128;
}

impl cfg_mocks::status_notification::pallet::Config for Runtime {
	type Id = (AccountId, SwapId);
	type Status = SwapState<Balance, Balance, CurrencyId>;
}

impl pallet_swaps::Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type FulfilledSwap = FulfilledSwapHook;
	type OrderBook = MockTokenSwaps;
	type OrderId = OrderId;
	type SwapId = SwapId;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	System::externalities()
}
