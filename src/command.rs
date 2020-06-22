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
use sc_cli::SubstrateCli;

impl SubstrateCli for Cli {
	fn impl_name() -> &'static str {
		"Centrifuge Chain Node"
	}

	fn impl_version() -> &'static str {
		env!("CARGO_PKG_VERSION")
	}

	fn description() -> &'static str { "Centrifuge Chain Node" }

	fn author() -> &'static str { "Centrifuge" }

	fn support_url() -> &'static str {
		"centrifuge.io"
	}

	fn copyright_start_year() -> i32 {
		2019
	}

	fn executable_name() -> &'static str {
		env!("CARGO_PKG_NAME")
	}

	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		chain_spec::load_spec(id)
	}
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(subcommand) => {
			let runner = cli.create_runner(subcommand)?;
			runner.run_subcommand(subcommand, |config| Ok(new_full_start!(config).0))
		}
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node(
				service::new_light,
				service::new_full,
				node_runtime::VERSION
			)
		}
	}
}

