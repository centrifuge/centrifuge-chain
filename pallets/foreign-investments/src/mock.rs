use cfg_types::investments::{ExecutedForeignCollect, ExecutedForeignDecreaseInvest};
use frame_support::derive_impl;
use sp_runtime::FixedU128;

use crate::{pallet as pallet_foreign_investments, FulfilledSwapHook, SwapId};

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
		MockDecreaseInvestHook: cfg_mocks::status_notification::pallet::<Instance1>,
		MockCollectInvestHook: cfg_mocks::status_notification::pallet::<Instance2>,
		MockCollectRedeemHook: cfg_mocks::status_notification::pallet::<Instance3>,
		MockPools: cfg_mocks::pools::pallet,
		Swaps: pallet_swaps::pallet,
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

type Hook1 = cfg_mocks::status_notification::pallet::Instance1;
impl cfg_mocks::status_notification::pallet::Config<Hook1> for Runtime {
	type Id = (AccountId, (PoolId, TrancheId));
	type Status = ExecutedForeignDecreaseInvest<Balance, CurrencyId>;
}

type Hook2 = cfg_mocks::status_notification::pallet::Instance2;
impl cfg_mocks::status_notification::pallet::Config<Hook2> for Runtime {
	type Id = (AccountId, (PoolId, TrancheId));
	type Status = ExecutedForeignCollect<Balance, Balance, Balance, CurrencyId>;
}

type Hook3 = cfg_mocks::status_notification::pallet::Instance3;
impl cfg_mocks::status_notification::pallet::Config<Hook3> for Runtime {
	type Id = (AccountId, (PoolId, TrancheId));
	type Status = ExecutedForeignCollect<Balance, Balance, Balance, CurrencyId>;
}

impl cfg_mocks::pools::pallet::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CurrencyId = CurrencyId;
	type PoolId = PoolId;
	type TrancheId = TrancheId;
}

impl pallet_swaps::Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type FulfilledSwap = FulfilledSwapHook<Runtime>;
	type OrderBook = MockTokenSwaps;
	type OrderId = OrderId;
	type SwapId = SwapId<Runtime>;
}

impl pallet_foreign_investments::Config for Runtime {
	type CollectedForeignInvestmentHook = MockCollectInvestHook;
	type CollectedForeignRedemptionHook = MockCollectRedeemHook;
	type CurrencyId = CurrencyId;
	type DecreasedForeignInvestOrderHook = MockDecreaseInvestHook;
	type ForeignBalance = Balance;
	type Investment = MockInvestment;
	type InvestmentId = (PoolId, TrancheId);
	type PoolBalance = Balance;
	type PoolInspect = MockPools;
	type RuntimeEvent = RuntimeEvent;
	type SwapBalance = Balance;
	type SwapRatio = Ratio;
	type Swaps = Swaps;
	type TrancheBalance = Balance;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	System::externalities()
}
