use cfg_mocks::{
	pallet_mock_currency_conversion, pallet_mock_investment, pallet_mock_pools,
	pallet_mock_status_notification, pallet_mock_token_swaps,
};
use cfg_traits::investments::TrancheCurrency;
use cfg_types::investments::{ExecutedForeignCollect, ExecutedForeignDecreaseInvest, Swap};
use frame_support::traits::{ConstU16, ConstU32, ConstU64};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	FixedU128,
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
pub type SwapId = u64;
pub type CurrencyId = u8;
pub type Ratio = FixedU128;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
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
		MockCollectInvestHook: pallet_mock_status_notification::<Instance3>,
		MockCurrencyConversion: pallet_mock_currency_conversion,
		MockPools: pallet_mock_pools,
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
	type OrderDetails = Swap<Balance, CurrencyId>;
	type OrderId = SwapId;
	type SellRatio = FixedU128;
}

type Hook1 = pallet_mock_status_notification::Instance1;
impl pallet_mock_status_notification::Config<Hook1> for Runtime {
	type Id = (AccountId, InvestmentId);
	type Status = ExecutedForeignDecreaseInvest<Balance, CurrencyId>;
}

type Hook2 = pallet_mock_status_notification::Instance2;
impl pallet_mock_status_notification::Config<Hook2> for Runtime {
	type Id = (AccountId, InvestmentId);
	type Status = ExecutedForeignCollect<Balance, CurrencyId>;
}

type Hook3 = pallet_mock_status_notification::Instance3;
impl pallet_mock_status_notification::Config<Hook3> for Runtime {
	type Id = (AccountId, InvestmentId);
	type Status = ExecutedForeignCollect<Balance, CurrencyId>;
}

impl pallet_mock_currency_conversion::Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
}

impl pallet_mock_pools::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CurrencyId = CurrencyId;
	type PoolId = PoolId;
	type TrancheCurrency = InvestmentId;
	type TrancheId = TrancheId;
}

impl pallet_foreign_investments::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CollectedForeignInvestmentHook = MockCollectInvestHook;
	type CollectedForeignRedemptionHook = MockCollectRedeemHook;
	type CurrencyConverter = MockCurrencyConversion;
	type CurrencyId = CurrencyId;
	type DecreasedForeignInvestOrderHook = MockDecreaseInvestHook;
	type Investment = MockInvestment;
	type InvestmentId = InvestmentId;
	type PoolId = PoolId;
	type PoolInspect = MockPools;
	type SwapId = SwapId;
	type TokenSwaps = MockTokenSwaps;
	type TrancheId = TrancheId;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	sp_io::TestExternalities::new(storage)
}
