pub mod asset_registry;
pub mod change_guard;
pub mod converter;
pub mod currency_conversion;
pub mod data;
pub mod ethereum_transactor;
pub mod fees;
pub mod foreign_investment;
pub mod foreign_investment_hooks;
pub mod investment;
pub mod liquidity_pools;
pub mod liquidity_pools_gateway;
pub mod liquidity_pools_gateway_queue;
pub mod pay_fee;
pub mod permissions;
pub mod pools;
pub mod pre_conditions;
pub mod rewards;
pub mod router_message;
pub mod status_notification;
pub mod time;
pub mod token_swaps;
pub mod value_provider;
pub mod write_off_policy;

pub use change_guard::pallet as pallet_mock_change_guard;
pub use currency_conversion::pallet as pallet_mock_currency_conversion;
pub use data::pallet as pallet_mock_data;
pub use fees::pallet as pallet_mock_fees;
pub use investment::pallet as pallet_mock_investment;
pub use liquidity_pools::pallet as pallet_mock_liquidity_pools;
pub use liquidity_pools_gateway::pallet as pallet_mock_liquidity_pools_gateway;
pub use liquidity_pools_gateway_queue::pallet as pallet_mock_liquidity_pools_gateway_queue;
pub use pay_fee::pallet as pallet_mock_pay_fee;
pub use permissions::pallet as pallet_mock_permissions;
pub use pools::pallet as pallet_mock_pools;
pub use pre_conditions::pallet as pallet_mock_pre_conditions;
pub use rewards::pallet as pallet_mock_rewards;
pub use status_notification::pallet as pallet_mock_status_notification;
pub use time::pallet as pallet_mock_time;
pub use token_swaps::pallet as pallet_mock_token_swaps;
pub use value_provider::pallet as pallet_mock_value_provider;
pub use write_off_policy::pallet as pallet_mock_write_off_policy;

#[cfg(test)]
#[allow(unused)]
mod template;

#[cfg(feature = "std")]
pub mod reexport {
	pub use frame_support;
	pub use frame_system;
	pub use sp_core;
	pub use sp_io;
	pub use sp_runtime;
}

/// Creates a runtime with a pallet mock to make isolated tests
/// See tests below of this same file
#[macro_export]
macro_rules! make_runtime_for_mock {
	($runtime_name:ident, $mock_name:ident, $pallet:ident, $externalities:ident) => {
		use $crate::reexport::{
			frame_support,
            frame_support::derive_impl,
			frame_system,
			sp_io,
		};

		frame_support::construct_runtime!(
			pub enum $runtime_name {
				System: frame_system,
				$mock_name: $pallet,
			}
		);

        #[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
        impl frame_system::Config for Runtime {
            type Block = frame_system::mocking::MockBlock<Runtime>;
        }

		pub fn $externalities() -> sp_io::TestExternalities {
			sp_io::TestExternalities::default()
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
