use frame_support::derive_impl;
use sp_runtime::FixedU128;

use crate::pallet as pallet_foreign_investments;

pub type AccountId = u64;
pub type Balance = u128;
pub type TrancheId = u32;
pub type PoolId = u64;
pub type OrderId = u64;
pub type CurrencyId = u8;
pub type Ratio = FixedU128;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		MockInvestment: cfg_mocks::investment::pallet,
		MockTokenSwaps: cfg_mocks::token_swaps::pallet,
		MockHooks: cfg_mocks::foreign_investment_hooks::pallet,
		MockPools: cfg_mocks::pools::pallet,
		ForeignInvestment: pallet_foreign_investments,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl cfg_mocks::investment::pallet::Config for Runtime {
	type Amount = Balance;
	type CurrencyId = CurrencyId;
	type InvestmentId = (PoolId, TrancheId);
	type TrancheAmount = Balance;
}

impl cfg_mocks::token_swaps::pallet::Config for Runtime {
	type BalanceIn = Balance;
	type BalanceOut = Balance;
	type CurrencyId = CurrencyId;
	type OrderId = OrderId;
	type Ratio = FixedU128;
}

impl cfg_mocks::foreign_investment_hooks::pallet::Config for Runtime {
	type Amount = Balance;
	type CurrencyId = CurrencyId;
	type InvestmentId = (PoolId, TrancheId);
	type TrancheAmount = Balance;
}

impl cfg_mocks::pools::pallet::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CurrencyId = CurrencyId;
	type PoolId = PoolId;
	type TrancheId = TrancheId;
}

impl pallet_foreign_investments::Config for Runtime {
	type CurrencyId = CurrencyId;
	type ForeignBalance = Balance;
	type Hooks = MockHooks;
	type Investment = MockInvestment;
	type OrderBook = MockTokenSwaps;
	type OrderId = OrderId;
	type InvestmentId = (PoolId, TrancheId);
	type PoolBalance = Balance;
	type PoolInspect = MockPools;
	type RuntimeEvent = RuntimeEvent;
	type SwapBalance = Balance;
	type SwapRatio = Ratio;
	type TrancheBalance = Balance;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	System::externalities()
}
