// Copyright 2019 Parity Technologies (UK) Ltd.
// This file is part of Cumulus.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

use std::path::PathBuf;

use clap::Parser;

use crate::{chain_spec, service::evm::EthConfiguration};

#[derive(Debug, Parser)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommand {
	/// Export the genesis state of the parachain.
	#[clap(name = "export-genesis-state")]
	ExportGenesisState(ExportGenesisStateCommand),

	/// Export the genesis wasm of the parachain.
	#[clap(name = "export-genesis-wasm")]
	ExportGenesisWasm(ExportGenesisWasmCommand),

	/// Build a chain specification.
	BuildSpec(sc_cli::BuildSpecCmd),

	/// Validate blocks.
	CheckBlock(sc_cli::CheckBlockCmd),

	/// Export blocks.
	ExportBlocks(sc_cli::ExportBlocksCmd),

	/// Export the state of a given block into a chain spec.
	ExportState(sc_cli::ExportStateCmd),

	/// Import blocks.
	ImportBlocks(sc_cli::ImportBlocksCmd),

	/// Remove the whole chain.
	PurgeChain(cumulus_client_cli::PurgeChainCmd),

	/// Revert the chain to a previous state.
	Revert(sc_cli::RevertCmd),

	/// Sub-commands concerned with benchmarking.
	/// The pallet benchmarking moved to the `pallet` sub-command.
	#[clap(subcommand)]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),

	/// Try some experimental command on the runtime. This includes migration
	/// and runtime-upgrade testing.
	#[cfg(feature = "try-runtime")]
	TryRuntime(try_runtime_cli::TryRuntimeCmd),
}

/// Command for exporting the genesis state of the parachain
#[derive(Debug, Parser)]
pub struct ExportGenesisStateCommand {
	/// Output file name or stdout if unspecified.
	#[clap(value_parser)]
	pub output: Option<PathBuf>,

	/// Write output in binary. Default is to write in hex.
	#[clap(short, long)]
	pub raw: bool,

	/// Id of the parachain this state is for.
	#[clap(long)]
	pub parachain_id: Option<u32>,

	/// The name of the chain for that the genesis state should be exported.
	#[clap(long)]
	pub chain: Option<String>,
}

/// Command for exporting the genesis wasm file.
#[derive(Debug, Parser)]
pub struct ExportGenesisWasmCommand {
	/// Output file name or stdout if unspecified.
	#[clap(value_parser)]
	pub output: Option<PathBuf>,

	/// Write output in binary. Default is to write in hex.
	#[clap(short, long)]
	pub raw: bool,

	/// The name of the chain for that the genesis wasm file should be exported.
	#[clap(long)]
	pub chain: Option<String>,
}

#[derive(Debug, Parser)]
#[clap(
	propagate_version = true,
	args_conflicts_with_subcommands = true,
	subcommand_negates_reqs = true
)]
pub struct Cli {
	#[clap(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[clap(flatten)]
	pub run: cumulus_client_cli::RunCmd,

	#[clap(flatten)]
	pub eth: EthConfiguration,

	/// Relaychain arguments
	#[clap(raw = true)]
	pub relaychain_args: Vec<String>,

	/// Id of the parachain the `run` execution may need.
	#[clap(long)]
	pub parachain_id: Option<u32>,
}

#[derive(Debug)]
pub struct RelayChainCli {
	/// The actual relay chain cli object.
	pub base: polkadot_cli::RunCmd,

	/// Optional chain id that should be passed to the relay chain.
	pub chain_id: Option<String>,

	/// The base path that should be used by the relay chain.
	pub base_path: Option<PathBuf>,
}

impl RelayChainCli {
	/// Parse the relay chain CLI parameters using the para chain
	/// `Configuration`.
	pub fn new<'a>(
		para_config: &sc_service::Configuration,
		relay_chain_args: impl Iterator<Item = &'a String>,
	) -> Self {
		let extension = chain_spec::Extensions::try_get(&*para_config.chain_spec);
		let chain_id = extension.map(|e| e.relay_chain.clone());
		let base_path = para_config.base_path.path().join("polkadot");
		Self {
			base_path: Some(base_path),
			chain_id,
			base: polkadot_cli::RunCmd::parse_from(relay_chain_args),
		}
	}
}
