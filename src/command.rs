// Copyright (C) 2017-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::chain_spec;
use crate::cli::Cli;
use crate::service;
use sc_service::PartialComponents;
use sc_cli::{SubstrateCli, RuntimeVersion, Role, ChainSpec};

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Centrifuge Chain Node".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String { "Centrifuge Chain Node".into() }

	fn author() -> String { "Centrifuge".into() }

	fn support_url() -> String {
		"centrifuge.io".into()
	}

	fn copyright_start_year() -> i32 {
		2019
	}

	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		chain_spec::load_spec(id)
	}

	fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		&node_runtime::VERSION
	}
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(subcommand) => {
			let runner = cli.create_runner(subcommand)?;
			runner.run_subcommand(subcommand, |config| {
				let PartialComponents { client, backend, task_manager, import_queue, ..}
					= service::new_partial(&config)?;
				Ok((client, backend, import_queue, task_manager))
			})
		}
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| match config.role {
				Role::Light => service::new_light(config),
				_ => service::new_full(config),
			})
		}
	}
}

