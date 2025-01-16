
frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Balances: pallet_balances,
		MockPools: pallet_mock_pools,
		MockPermissions: pallet_mock_permissions,
		MockPrices: pallet_mock_data,
		MockChangeGuard: pallet_mock_change_guard,
		Loans: pallet_loans,
	}
);

frame_support::parameter_types! {
	pub const DomainAccount:  = 4;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
    type AccountData = pallet_balances::AccountData<Balance>;
    type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl cfg_mocks::time::pallet::Config for Runtime {
    type Moment = Millis;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
    type AccountStore = System;
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type RuntimeHoldReason = ();
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type DomainAccount = DomainAccount;
    type IouCfg = IouCfg;
    type EVMChainId = EVMChainId;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();
    t.into()
}
