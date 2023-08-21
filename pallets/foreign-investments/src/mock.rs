use cfg_mocks::{
	pallet_mock_currency_conversion, pallet_mock_investment, pallet_mock_status_notification,
	pallet_mock_token_swaps,
};
use cfg_traits::investments::TrancheCurrency;
use cfg_types::investments::{
	ExecutedForeignCollectRedeem, ExecutedForeignDecrease, ForeignInvestmentInfo,
};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::{ConstU128, ConstU16, ConstU32, ConstU64};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::pallet as pallet_foreign_investments;

// =============
//     Types
// =============

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub type AccountId = u64;
pub type Balance = u128;
pub type TrancheId = u32;
pub type PoolId = u64;
pub type OrderId = u64;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub enum CurrencyId {
	Tranche(PoolId, TrancheId),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct InvestmentId(PoolId, TrancheId);

impl From<InvestmentId> for CurrencyId {
	fn from(investment: InvestmentId) -> Self {
		CurrencyId::Tranche(investment.0, investment.1)
	}
}

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

// ======================
//     Runtime config
// ======================

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		MockInvestment: pallet_mock_investment,
		MockTokenSwaps: pallet_mock_token_swaps,
		MockDecreaseInvestHook: pallet_mock_status_notification::<Instance1>,
		MockCollectRedeemHook: pallet_mock_status_notification::<Instance2>,
		MockCurrencyConversion: pallet_mock_currency_conversion,
		ForeignInvestment: pallet_foreign_investments,
	}
);

impl frame_system::Config for Runtime {
	type AccountData = ();
	type AccountId = AccountId;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = ConstU64<250>;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
	type DbWeight = ();
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = ConstU16<42>;
	type SystemWeightInfo = ();
	type Version = ();
}

impl pallet_mock_investment::Config for Runtime {
	type Amount = Balance;
	type CurrencyId = CurrencyId;
	type InvestmentId = InvestmentId;
}

impl pallet_mock_token_swaps::Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type OrderId = OrderId;
}

type Hook1 = pallet_mock_status_notification::Instance1;
impl pallet_mock_status_notification::Config<Hook1> for Runtime {
	type Id = ForeignInvestmentInfo<AccountId, InvestmentId, ()>;
	type Status = ExecutedForeignDecrease<Balance, CurrencyId>;
}

type Hook2 = pallet_mock_status_notification::Instance2;
impl pallet_mock_status_notification::Config<Hook2> for Runtime {
	type Id = ForeignInvestmentInfo<AccountId, InvestmentId, ()>;
	type Status = ExecutedForeignCollectRedeem<Balance, CurrencyId>;
}

impl pallet_mock_currency_conversion::Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
}

impl pallet_foreign_investments::Config for Runtime {
	type Balance = Balance;
	type CurrencyConverter = MockCurrencyConversion;
	type CurrencyId = CurrencyId;
	type DefaultTokenMinFulfillmentAmount = ConstU128<1>;
	type DefaultTokenSwapSellPriceLimit = ConstU128<1>;
	type ExecutedCollectRedeemHook = MockCollectRedeemHook;
	type ExecutedDecreaseInvestHook = MockDecreaseInvestHook;
	type Investment = MockInvestment;
	type InvestmentId = InvestmentId;
	type PoolId = PoolId;
	type RuntimeEvent = RuntimeEvent;
	type TokenSwapOrderId = OrderId;
	type TokenSwaps = MockTokenSwaps;
	type TrancheId = TrancheId;
	type WeightInfo = ();
}
