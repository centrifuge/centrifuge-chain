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

use cumulus_primitives_core::ParaId;
use frame_support::traits::GenesisBuild;
use polkadot_primitives::v1::{BlockNumber, MAX_CODE_SIZE, MAX_POV_SIZE};
use polkadot_runtime_parachains::configuration::HostConfiguration;
use sp_runtime::traits::AccountIdConversion;
use xcm_emulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};

use development_runtime::CurrencyId;
use runtime_common::AccountId;

use crate::{
	chain::centrifuge::PARA_ID,
	xcm::setup::{ksm_amount, native_amount, ExtBuilder, ALICE, BOB, PARA_ID_DEVELOPMENT, PARA_ID_KARURA,
				 PARA_ID_SIBLING,}
};

decl_test_relay_chain! {
	pub struct RelayNet {
		Runtime = crate::chain::relay::Runtime,
		XcmConfig = crate::chain::relay::xcm_config::XcmConfig,
		new_ext = relay_ext(),
	}
}

decl_test_parachain! {
	pub struct Centrifuge {
		Runtime = crate::chain::centrifuge::Runtime,
		Origin = crate::chain::centrifuge::Origin,
		XcmpMessageHandler = crate::chain::centrifuge::XcmpQueue,
		DmpMessageHandler = crate::chain::centrifuge::DmpQueue,
		new_ext = para_ext(PARA_ID),
	}
}

decl_test_parachain! {
	pub struct Development {
		Runtime = development_runtime::Runtime,
		Origin = development_runtime::Origin,
		XcmpMessageHandler = development_runtime::XcmpQueue,
		DmpMessageHandler = development_runtime::DmpQueue,
		new_ext = para_ext(PARA_ID_DEVELOPMENT),
	}
}

decl_test_parachain! {
	pub struct Sibling {
		Runtime = development_runtime::Runtime,
		Origin = development_runtime::Origin,
		XcmpMessageHandler = development_runtime::XcmpQueue,
		DmpMessageHandler = development_runtime::DmpQueue,
		new_ext = para_ext(PARA_ID_SIBLING),
	}
}

decl_test_parachain! {
	pub struct Karura {
		Runtime = development_runtime::Runtime,
		Origin = development_runtime::Origin,
		XcmpMessageHandler = development_runtime::XcmpQueue,
		DmpMessageHandler = development_runtime::DmpQueue,
		new_ext = para_ext(PARA_ID_KARURA),
	}
}

decl_test_network! {
	pub struct TestNet {
		relay_chain = RelayNet,
		parachains = vec![
			// N.B: Ideally, we could use the defined para id constants but doing so
			// fails with: "error: arbitrary expressions aren't allowed in patterns"

			// Be sure to use `PARA_ID`
			(2031, Centrifuge),
			// Be sure to use `PARA_ID_DEVELOPMENT`
			(2088, Development),
			// Be sure to use `PARA_ID_SIBLING`
			(3000, Sibling),
			// Be sure to use `PARA_ID_KARURA`
			(2000, Karura),
		],
	}
}
//
// decl_test_network! {
// 	pub struct KusamaNet {
// 		relay_chain = RelayNet,
// 		parachains = vec![
// 			// N.B: Ideally, we could use the defined para id constants but doing so
// 			// fails with: "error: arbitrary expressions aren't allowed in patterns"
//
// 			// Be sure to use `PARA_ID_DEVELOPMENT`
// 			(2088, Development),
// 			// Be sure to use `PARA_ID_SIBLING`
// 			(3000, Sibling),
// 			// Be sure to use `PARA_ID_KARURA`
// 			(2000, Karura),
// 		],
// 	}
// }

pub fn relay_ext() -> sp_io::TestExternalities {
	use crate::chain::relay::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![
			(AccountId::from(ALICE), native_amount(2002)),
			(ParaId::from(PARA_ID).into_account(), native_amount(7)),
			(
				ParaId::from(PARA_ID_SIBLING).into_account(),
				native_amount(7),
			),
		],
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
			(
				AccountId::from(ALICE),
				CurrencyId::Native,
				native_amount(10),
			),
			(AccountId::from(BOB), CurrencyId::Native, native_amount(10)),
			(AccountId::from(ALICE), CurrencyId::KSM, ksm_amount(10)),
			(
				development_runtime::TreasuryAccount::get(),
				CurrencyId::KSM,
				ksm_amount(1),
			),
		])
		.parachain_id(parachain_id)
		.build()
}

fn default_parachains_host_configuration() -> HostConfiguration<BlockNumber> {
	HostConfiguration {
		minimum_validation_upgrade_delay: 5,
		validation_upgrade_cooldown: 5u32,
		validation_upgrade_delay: 5,
		code_retention_period: 1200,
		max_code_size: MAX_CODE_SIZE,
		max_pov_size: MAX_POV_SIZE,
		max_head_data_size: 32 * 1024,
		group_rotation_frequency: 20,
		chain_availability_period: 4,
		thread_availability_period: 4,
		max_upward_queue_count: 8,
		max_upward_queue_size: 1024 * 1024,
		max_downward_message_size: 1024,
		ump_service_total_weight: 4 * 1_000_000_000,
		max_upward_message_size: 1024 * 1024,
		max_upward_message_num_per_candidate: 5,
		hrmp_sender_deposit: 0,
		hrmp_recipient_deposit: 0,
		hrmp_channel_max_capacity: 8,
		hrmp_channel_max_total_size: 8 * 1024,
		hrmp_max_parachain_inbound_channels: 4,
		hrmp_max_parathread_inbound_channels: 4,
		hrmp_channel_max_message_size: 1024 * 1024,
		hrmp_max_parachain_outbound_channels: 4,
		hrmp_max_parathread_outbound_channels: 4,
		hrmp_max_message_num_per_candidate: 5,
		dispute_period: 6,
		no_show_slots: 2,
		n_delay_tranches: 25,
		needed_approvals: 2,
		relay_vrf_modulo_samples: 2,
		zeroth_delay_tranche_width: 0,
		..Default::default()
	}
}
