use sp_core::H256;
use crate::va_registry::{Module, Trait};
use crate::{anchor, nft, fees, va_registry};
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use sp_runtime::{
    traits::{Block as BlockT, BlakeTwo256, IdentityLookup}, testing::Header, Perbill,
};

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl frame_system::Trait for Test {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type ModuleToIndex = ();
    type AccountData = pallet_balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

impl crate::nft::Trait for Test {
    type Event = Event;
    type AssetInfo = crate::va_registry::types::AssetInfo;
}

impl crate::anchor::Trait for Test {}

impl pallet_timestamp::Trait for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ();
    type WeightInfo = ();
}

impl crate::fees::Trait for Test {
    type Event = Event;
    type FeeChangeOrigin = frame_system::EnsureRoot<u64>;
}

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
}
impl pallet_balances::Trait for Test {
    type Balance = u64;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = frame_system::Module<Test>;
    type WeightInfo = ();
}

impl pallet_authorship::Trait for Test {
    type FindAuthor = ();
    type UncleGenerations = ();
    type FilterUncle = ();
    type EventHandler = ();
}

impl Trait for Test {
    type Event = Event;
}

// System Under Test
pub type SUT = Module<Test>;

pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<u64, Call, u64, ()>;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
        Anchor: anchor::{Module, Call, Storage},
        Fees: fees::{Module, Event<T>},
        Nft: nft::{Module, Event<T>},
        Registry: va_registry::{Module, Call, Event<T>},
        Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
        Authorship: pallet_authorship::{Module, Call, Storage, Inherent},
    }
);

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
    crate::fees::GenesisConfig::<Test> {
        initial_fees: vec![(
            // anchoring state rent fee per day
            H256::from(&[
                17, 218, 109, 31, 118, 29, 223, 155, 219, 76, 157, 110, 83, 3, 235, 212, 31, 97,
                133, 141, 10, 86, 71, 161, 167, 191, 224, 137, 191, 146, 27, 233,
            ]),
            // state rent 0 for tests
            0,
        )],
    }
    .assimilate_storage(&mut t)
    .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| frame_system::Module::<Test>::set_block_number(1));
    ext
}
