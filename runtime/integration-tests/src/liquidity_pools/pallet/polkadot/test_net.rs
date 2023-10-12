// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Relay chain and parachains emulation.

use centrifuge_runtime::CurrencyId;
use cfg_primitives::{parachains, AccountId};
use cumulus_primitives_core::ParaId;
use frame_support::{traits::GenesisBuild, weights::Weight};
use polkadot_primitives::{BlockNumber, MAX_CODE_SIZE, MAX_POV_SIZE};
use polkadot_runtime_parachains::configuration::HostConfiguration;
use sp_runtime::traits::AccountIdConversion;
use xcm_simulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain, TestExt};

use super::setup::{cfg, dot, ExtBuilder, ALICE, BOB, DOT_ASSET_ID, PARA_ID_SIBLING};

decl_test_relay_chain! {
	pub struct PolkadotNet {
		Runtime = polkadot_runtime::Runtime,
		RuntimeCall = polkadot_runtime::RuntimeCall,
		RuntimeEvent = polkadot_runtime::RuntimeEvent,
		XcmConfig = polkadot_runtime::xcm_config::XcmConfig,
		MessageQueue = polkadot_runtime::MessageQueue,
		System = polkadot_runtime::System,
		new_ext = relay_ext(),
	}
}

decl_test_parachain! {
	pub struct Centrifuge {
		Runtime = centrifuge_runtime::Runtime,
		XcmpMessageHandler = centrifuge_runtime::XcmpQueue,
		DmpMessageHandler = centrifuge_runtime::DmpQueue,
		new_ext = para_ext(parachains::polkadot::centrifuge::ID),
	}
}

decl_test_parachain! {
	pub struct Sibling {
		Runtime = centrifuge_runtime::Runtime,
		XcmpMessageHandler = centrifuge_runtime::XcmpQueue,
		DmpMessageHandler = centrifuge_runtime::DmpQueue,
		new_ext = para_ext(PARA_ID_SIBLING),
	}
}

decl_test_parachain! {
	pub struct Acala {
		Runtime = centrifuge_runtime::Runtime,
		XcmpMessageHandler = centrifuge_runtime::XcmpQueue,
		DmpMessageHandler = centrifuge_runtime::DmpQueue,
		new_ext = para_ext(parachains::polkadot::acala::ID),
	}
}

decl_test_network! {
	pub struct TestNet {
		relay_chain = PolkadotNet,
		parachains = vec![
			// N.B: Ideally, we could use the defined para id constants but doing so
			// fails with: "error: arbitrary expressions aren't allowed in patterns"

			// Be sure to use `parachains::polkadot::centrifuge::ID`
			(2031, Centrifuge),
			// Be sure to use `PARA_ID_SIBLING`
			(3000, Sibling),
			// Be sure to use `parachains::polkadot::acala::ID`
			(2000, Acala),
		],
	}
}

pub fn relay_ext() -> sp_io::TestExternalities {
	use polkadot_runtime::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(AccountId::from(ALICE), dot(10))],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	polkadot_runtime_parachains::configuration::GenesisConfig::<Runtime> {
		config: default_parachains_host_configuration(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	<pallet_xcm::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
		&pallet_xcm::GenesisConfig {
			safe_xcm_version: Some(2),
		},
		&mut t,
	)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn para_ext(parachain_id: u32) -> sp_io::TestExternalities {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), CurrencyId::Native, cfg(10)),
			(AccountId::from(BOB), CurrencyId::Native, cfg(10)),
		])
		.parachain_id(parachain_id)
		.build()
}

fn default_parachains_host_configuration() -> HostConfiguration<BlockNumber> {
	HostConfiguration {
		hrmp_channel_max_capacity: u32::MAX,
		hrmp_channel_max_total_size: u32::MAX,
		hrmp_max_parachain_inbound_channels: 10,
		hrmp_max_parachain_outbound_channels: 10,
		hrmp_channel_max_message_size: u32::MAX,
		// Changed to avoid aritmetic errors within hrmp_close
		max_downward_message_size: 100_000u32,
		..Default::default()
	}
}
