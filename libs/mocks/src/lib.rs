mod change_guard;
mod fees;
mod permissions;
mod pools;
mod rewards;

pub use change_guard::pallet_mock_change_guard;
pub use fees::pallet as pallet_mock_fees;
pub use permissions::pallet as pallet_mock_permissions;
pub use pools::pallet as pallet_mock_pools;
pub use rewards::pallet as pallet_mock_rewards;

#[cfg(test)]
#[allow(unused)]
mod template;

/// To use this macro, the following dependencies are needed:
/// - codec
/// - frame-support
/// - frame-system
/// - scale-info
/// - sp-core
/// - sp-io
/// - sp-runtime
#[macro_export]
macro_rules! make_runtime_for_mock {
	($runtime_name:ident, $mock_name:ident, $pallet:ident, $externalities:ident) => {
        use frame_support::traits::{ConstU16, ConstU32, ConstU64};
        use sp_core::H256;
        use sp_runtime::{
            testing::Header,
            traits::{BlakeTwo256, IdentityLookup},
        };

        type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
        type Block = frame_system::mocking::MockBlock<Runtime>;

        frame_support::construct_runtime!(
            pub enum $runtime_name where
                Block = Block,
                NodeBlock = Block,
                UncheckedExtrinsic = UncheckedExtrinsic,
            {
                System: frame_system,
                $mock_name: $pallet,
            }
        );

        impl frame_system::Config for Runtime {
            type AccountData = ();
            type AccountId = u64;
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

        pub fn $externalities() -> sp_io::TestExternalities {
            frame_system::GenesisConfig::default()
                .build_storage::<Runtime>()
                .unwrap()
                .into()
        }
	};
}

#[cfg(test)]
mod test {
	use template::pallet as pallet_mock_template;

	use super::*;

	make_runtime_for_mock!(Runtime, Mock, pallet_mock_template, new_test_ext);

	impl pallet_mock_template::Config for Runtime {
		// Configure your associated types here
	}

	#[test]
	fn runtime_for_mock() {
		new_test_ext().execute_with(|| {
			// Test using the Mock
		});
	}
}
