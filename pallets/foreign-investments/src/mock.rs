use cfg_traits::investments::TrancheCurrency;
use cfg_types::investments::{ExecutedForeignCollect, ExecutedForeignDecreaseInvest};
use frame_support::derive_impl;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::FixedU128;

use crate::{pallet as pallet_foreign_investments, FulfilledSwapHook, SwapId};

pub type AccountId = u64;
pub type Balance = u128;
pub type TrancheId = u32;
pub type PoolId = u64;
pub type OrderId = u64;
pub type CurrencyId = u8;
pub type Ratio = FixedU128;

#[derive(
	Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
pub struct InvestmentId(pub PoolId, pub TrancheId);

impl TrancheCurrency<PoolId, TrancheId> for InvestmentId {
	fn generate(pool_id: PoolId, tranche_id: TrancheId) -> Self {
		Self(pool_id, tranche_id)
	}

	fn of_pool(&self) -> PoolId {
		self.0
	}

	fn of_tranche(&self) -> TrancheId {
		self.1
	}
}

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
	type InvestmentId = InvestmentId;
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
	type Id = (AccountId, InvestmentId);
	type Status = ExecutedForeignDecreaseInvest<Balance, CurrencyId>;
}

type Hook2 = cfg_mocks::status_notification::pallet::Instance2;
impl cfg_mocks::status_notification::pallet::Config<Hook2> for Runtime {
	type Id = (AccountId, InvestmentId);
	type Status = ExecutedForeignCollect<Balance, Balance, Balance, CurrencyId>;
}

type Hook3 = cfg_mocks::status_notification::pallet::Instance3;
impl cfg_mocks::status_notification::pallet::Config<Hook3> for Runtime {
	type Id = (AccountId, InvestmentId);
	type Status = ExecutedForeignCollect<Balance, Balance, Balance, CurrencyId>;
}

impl cfg_mocks::pools::pallet::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CurrencyId = CurrencyId;
	type PoolId = PoolId;
	type TrancheCurrency = InvestmentId;
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
	type InvestmentId = InvestmentId;
	type PoolBalance = Balance;
	type PoolInspect = MockPools;
	type SwapBalance = Balance;
	type Swaps = Swaps;
	type TrancheBalance = Balance;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	sp_io::TestExternalities::default()
}
