use cfg_traits::swaps::SwapInfo;
use frame_support::traits::{ConstU16, ConstU32, ConstU64};
use sp_runtime::{
	testing::{Header, H256},
	traits::{BlakeTwo256, IdentityLookup},
	FixedU128,
};

use crate::pallet as pallet_swaps;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub type AccountId = u64;
pub type Balance = u128;
pub type OrderId = u64;
pub type SwapId = u32;
pub type CurrencyId = u8;
pub type Ratio = FixedU128;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		MockTokenSwaps: cfg_mocks::token_swaps::pallet,
		FulfilledSwapHook: cfg_mocks::status_notification::pallet,
		Swaps: pallet_swaps,
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

impl cfg_mocks::token_swaps::pallet::Config for Runtime {
	type BalanceIn = Balance;
	type BalanceOut = Balance;
	type CurrencyId = CurrencyId;
	type OrderId = OrderId;
	type Ratio = Ratio;
}

impl cfg_mocks::status_notification::pallet::Config for Runtime {
	type Id = (AccountId, SwapId);
	type Status = SwapInfo<Balance, Balance, CurrencyId, Ratio>;
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
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	sp_io::TestExternalities::new(storage)
}
