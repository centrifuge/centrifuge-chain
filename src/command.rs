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

use std::{io::Write, net::SocketAddr};

use cfg_primitives::Block;
use codec::Encode;
use cumulus_client_cli::generate_genesis_block;
use cumulus_primitives_core::ParaId;
use frame_benchmarking_cli::{BenchmarkCmd, SUBSTRATE_REFERENCE_HARDWARE};
use log::{info, warn};
use sc_cli::{
	ChainSpec, CliConfiguration, DefaultConfigurationValues, ImportParams, KeystoreParams,
	NetworkParams, Result, RuntimeVersion, SharedParams, SubstrateCli,
};
use sc_service::config::{BasePath, PrometheusConfig};
use sp_core::hexdisplay::HexDisplay;
use sp_runtime::traits::{AccountIdConversion, Block as BlockT};

use crate::{
	chain_spec,
	cli::{Cli, RelayChainCli, Subcommand},
	service::{
		evm::new_partial, AltairRuntimeExecutor, CentrifugeRuntimeExecutor,
		DevelopmentRuntimeExecutor,
	},
};

enum ChainIdentity {
	Altair,
	Centrifuge,
	Development,
}

trait IdentifyChain {
	fn identify(&self) -> ChainIdentity;
}

impl IdentifyChain for dyn sc_service::ChainSpec {
	fn identify(&self) -> ChainIdentity {
		if self.id().starts_with("centrifuge") || self.id().starts_with("catalyst") {
			ChainIdentity::Centrifuge
		} else if self.id().starts_with("altair")
			|| self.id().starts_with("charcoal")
			|| self.id().starts_with("antares")
		{
			ChainIdentity::Altair
		} else {
			ChainIdentity::Development
		}
	}
}

impl<T: sc_service::ChainSpec + 'static> IdentifyChain for T {
	fn identify(&self) -> ChainIdentity {
		<dyn sc_service::ChainSpec>::identify(self)
	}
}

fn load_spec(
	id: &str,
	para_id: ParaId,
) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
	match id {
		"centrifuge" => Ok(Box::new(chain_spec::centrifuge_config())),
		"centrifuge-staging" => Ok(Box::new(chain_spec::centrifuge_staging(para_id))),
		"centrifuge-dev" => Ok(Box::new(chain_spec::centrifuge_dev(para_id))),
		"centrifuge-local" => Ok(Box::new(chain_spec::centrifuge_local(para_id))),
		"altair" => Ok(Box::new(chain_spec::altair_config())),
		"altair-staging" => Ok(Box::new(chain_spec::altair_staging(para_id))),
		"altair-dev" => Ok(Box::new(chain_spec::altair_dev(para_id))),
		"altair-local" => Ok(Box::new(chain_spec::altair_local(para_id))),
		"algol" => Ok(Box::new(chain_spec::algol_config())),
		"catalyst" => Ok(Box::new(chain_spec::catalyst_config())),
		"catalyst-staging" => Ok(Box::new(chain_spec::catalyst_staging(para_id))),
		"catalyst-local" => Ok(Box::new(chain_spec::catalyst_local(para_id))),
		"antares" => Ok(Box::new(chain_spec::antares_config())),
		"antares-staging" => Ok(Box::new(chain_spec::antares_staging(para_id))),
		"antares-local" => Ok(Box::new(chain_spec::antares_local(para_id))),
		"charcoal" => Ok(Box::new(chain_spec::charcoal_config())),
		"charcoal-staging" => Ok(Box::new(chain_spec::charcoal_staging(para_id))),
		"charcoal-local" => Ok(Box::new(chain_spec::charcoal_local(para_id))),
		"demo" => Ok(Box::new(chain_spec::demo(para_id))),
		"development" => Ok(Box::new(chain_spec::development(para_id))),
		"development-local" => Ok(Box::new(chain_spec::development_local(para_id))),
		"" => Err(String::from("No Chain-id provided")),

		path => {
			let chain_spec = chain_spec::CentrifugeChainSpec::from_json_file(path.into())?;
			Ok(match chain_spec.identify() {
				ChainIdentity::Altair => {
					Box::new(chain_spec::AltairChainSpec::from_json_file(path.into())?)
				}
				ChainIdentity::Centrifuge => Box::new(
					chain_spec::CentrifugeChainSpec::from_json_file(path.into())?,
				),
				ChainIdentity::Development => Box::new(
					chain_spec::DevelopmentChainSpec::from_json_file(path.into())?,
				),
			})
		}
	}
}

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Centrifuge Parachain Collator".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		format!(
			"Cumulus test parachain collator\n\nThe command-line arguments provided first will be \
		passed to the parachain node, while the arguments provided after -- will be passed \
		to the relaychain node.\n\n\
		{} [parachain-args] -- [relaychain-args]",
			Self::executable_name()
		)
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/paritytech/cumulus/issues/new".into()
	}

	fn copyright_start_year() -> i32 {
		2017
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		load_spec(id, self.parachain_id.unwrap_or(10001).into())
	}

	fn native_runtime_version(spec: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		match spec.identify() {
			ChainIdentity::Altair => &altair_runtime::VERSION,
			ChainIdentity::Centrifuge => &centrifuge_runtime::VERSION,
			ChainIdentity::Development => &development_runtime::VERSION,
		}
	}
}

impl SubstrateCli for RelayChainCli {
	fn impl_name() -> String {
		"Cumulus Test Parachain Collator".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		"Cumulus test parachain collator\n\nThe command-line arguments provided first will be \
		passed to the parachain node, while the arguments provided after -- will be passed \
		to the relaychain node.\n\n\
		rococo-collator [parachain-args] -- [relaychain-args]"
			.into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/paritytech/cumulus/issues/new".into()
	}

	fn copyright_start_year() -> i32 {
		2017
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		polkadot_cli::Cli::from_iter([RelayChainCli::executable_name()].iter()).load_spec(id)
	}

	fn native_runtime_version(chain_spec: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		polkadot_cli::Cli::native_runtime_version(chain_spec)
	}
}

fn extract_genesis_wasm(chain_spec: &dyn sc_service::ChainSpec) -> Result<Vec<u8>> {
	let mut storage = chain_spec.build_storage()?;

	storage
		.top
		.remove(sp_core::storage::well_known_keys::CODE)
		.ok_or_else(|| "Could not find wasm file in genesis state!".into())
}

#[cfg(feature = "try-runtime")]
macro_rules! with_runtime {
	($chain_spec:expr, { $( $code:tt )* }) => {
		match $chain_spec.identify() {
			ChainIdentity::Altair => {
				use AltairRuntimeExecutor as Executor;
				$( $code )*
			}
			ChainIdentity::Centrifuge => {
				use CentrifugeRuntimeExecutor as Executor;
				$( $code )*
			}
			ChainIdentity::Development => {
				use DevelopmentRuntimeExecutor as Executor;
				$( $code )*
			}
		}
	}
}

macro_rules! construct_async_run {
	(|$components:ident, $cli:ident, $cmd:ident, $config:ident| $( $code:tt )* ) => {{
	    let runner = $cli.create_runner($cmd)?;
            match runner.config().chain_spec.identify() {
                ChainIdentity::Altair => {
		    runner.async_run(|$config| {
				let $components = new_partial::<altair_runtime::RuntimeApi, _>(
					&$config,
					crate::service::build_altair_import_queue,
				)?;
				let task_manager = $components.task_manager;
				{ $( $code )* }.map(|v| (v, task_manager))
		    })
                }
                ChainIdentity::Centrifuge => {
		    runner.async_run(|$config| {
				let $components = new_partial::<centrifuge_runtime::RuntimeApi, _>(
					&$config,
					crate::service::build_centrifuge_import_queue,
				)?;
				let task_manager = $components.task_manager;
				{ $( $code )* }.map(|v| (v, task_manager))
		    })
                }
                ChainIdentity::Development => {
		    runner.async_run(|$config| {
				let $components = new_partial::<development_runtime::RuntimeApi, _>(
					&$config,
					crate::service::build_development_import_queue,
				)?;
				let task_manager = $components.task_manager;
				{ $( $code )* }.map(|v| (v, task_manager))
		    })
                }
            }
	}}
}

/// Parse command line arguments into service configuration.
pub fn run() -> Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		}
		Some(Subcommand::CheckBlock(cmd)) => {
			construct_async_run!(|components, cli, cmd, config| {
				Ok(cmd.run(components.client, components.import_queue))
			})
		}
		Some(Subcommand::ExportBlocks(cmd)) => {
			construct_async_run!(|components, cli, cmd, config| {
				Ok(cmd.run(components.client, config.database))
			})
		}
		Some(Subcommand::ExportState(cmd)) => {
			construct_async_run!(|components, cli, cmd, config| {
				Ok(cmd.run(components.client, config.chain_spec))
			})
		}
		Some(Subcommand::ImportBlocks(cmd)) => {
			construct_async_run!(|components, cli, cmd, config| {
				Ok(cmd.run(components.client, components.import_queue))
			})
		}
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| {
				let polkadot_cli = RelayChainCli::new(
					&config,
					[RelayChainCli::executable_name()]
						.iter()
						.chain(cli.relaychain_args.iter()),
				);

				let polkadot_config = SubstrateCli::create_configuration(
					&polkadot_cli,
					&polkadot_cli,
					config.tokio_handle.clone(),
				)
				.map_err(|err| format!("Relay chain argument error: {err}"))?;

				cmd.run(config, polkadot_config)
			})
		}
		Some(Subcommand::Revert(cmd)) => construct_async_run!(|components, cli, cmd, config| {
			let aux_revert = Box::new(move |client, _, blocks| {
				grandpa::revert(client, blocks)?;
				Ok(())
			});
			Ok(cmd.run(components.client, components.backend, Some(aux_revert)))
		}),
		Some(Subcommand::ExportGenesisState(params)) => {
			let mut builder = sc_cli::LoggerBuilder::new("");
			builder.with_profiling(sc_tracing::TracingReceiver::Log, "");
			let _ = builder.init();

			let chain_spec = &load_spec(
				&params.chain.clone().unwrap_or_default(),
				params.parachain_id.unwrap_or(10001).into(),
			)?;

			let state_version = Cli::native_runtime_version(chain_spec).state_version();
			let block: Block = generate_genesis_block(&**chain_spec, state_version)?;

			let raw_header = block.header().encode();
			let output_buf = if params.raw {
				raw_header
			} else {
				format!("0x{:?}", HexDisplay::from(&block.header().encode())).into_bytes()
			};

			if let Some(output) = &params.output {
				std::fs::write(output, output_buf)?;
			} else {
				std::io::stdout().write_all(&output_buf)?;
			}

			Ok(())
		}
		Some(Subcommand::ExportGenesisWasm(params)) => {
			let mut builder = sc_cli::LoggerBuilder::new("");
			builder.with_profiling(sc_tracing::TracingReceiver::Log, "");
			let _ = builder.init();

			let raw_wasm_blob = extract_genesis_wasm(
				cli.load_spec(&params.chain.clone().unwrap_or_default())?
					.as_ref(),
			)?;
			let output_buf = if params.raw {
				raw_wasm_blob
			} else {
				format!("0x{:?}", HexDisplay::from(&raw_wasm_blob)).into_bytes()
			};

			if let Some(output) = &params.output {
				std::fs::write(output, output_buf)?;
			} else {
				std::io::stdout().write_all(&output_buf)?;
			}

			Ok(())
		}

		#[cfg(feature = "try-runtime")]
		Some(Subcommand::TryRuntime(cmd)) => {
			use sc_executor::{sp_wasm_interface::ExtendedHostFunctions, NativeExecutionDispatch};

			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;

			with_runtime!(chain_spec, {
				runner.async_run(|config| {
					let registry = config.prometheus_config.as_ref().map(|cfg| &cfg.registry);
					let task_manager =
						sc_service::TaskManager::new(config.tokio_handle.clone(), registry)
							.map_err(|e| {
								sc_cli::Error::Service(sc_service::Error::Prometheus(e))
							})?;
					Ok((
						cmd.run::<Block, ExtendedHostFunctions<
							sp_io::SubstrateHostFunctions,
							<Executor as NativeExecutionDispatch>::ExtendHostFunctions,
						>>(),
						task_manager,
					))
				})
			})
		}

		Some(Subcommand::Benchmark(cmd)) => {
			if cfg!(feature = "runtime-benchmarks") {
				let runner = cli.create_runner(cmd)?;

				// Handle the exact benchmark sub-command accordingly
				match cmd {
					BenchmarkCmd::Pallet(cmd) => match runner.config().chain_spec.identify() {
						ChainIdentity::Altair => runner.sync_run(|config| {
							cmd.run::<altair_runtime::Block, AltairRuntimeExecutor>(config)
						}),
						ChainIdentity::Centrifuge => runner.sync_run(|config| {
							cmd.run::<centrifuge_runtime::Block, CentrifugeRuntimeExecutor>(config)
						}),
						ChainIdentity::Development => runner.sync_run(|config| {
							cmd.run::<development_runtime::Block, DevelopmentRuntimeExecutor>(
								config,
							)
						}),
					},
					BenchmarkCmd::Block(_)
					| BenchmarkCmd::Storage(_)
					| BenchmarkCmd::Extrinsic(_)
					| BenchmarkCmd::Overhead(_) => Err("Unsupported benchmarking command".into()),
					BenchmarkCmd::Machine(cmd) => runner
						.sync_run(|config| cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone())),
				}
			} else {
				Err("Benchmarking wasn't enabled when building the node. \
				You can enable it with `--features runtime-benchmarks`."
					.into())
			}
		}
		None => {
			let runner = cli.create_runner(&cli.run.normalize())?;
			let collator_options = cli.run.collator_options();

			runner.run_node_until_exit(|config| async move {
				let polkadot_cli = RelayChainCli::new(
					&config,
					[RelayChainCli::executable_name()]
						.iter()
						.chain(cli.relaychain_args.iter()),
				);

				let para_id = chain_spec::Extensions::try_get(&*config.chain_spec)
					.map(|e| e.para_id).unwrap_or_else(|| cli.parachain_id.unwrap_or(10001));

				let id = ParaId::from(para_id);

				let parachain_account =
					AccountIdConversion::<polkadot_primitives::v2::AccountId>::into_account_truncating(&id);

				let state_version = Cli::native_runtime_version(&config.chain_spec).state_version();
				let block: Block = generate_genesis_block(&*config.chain_spec, state_version)
					.map_err(|e| format!("{e:?}"))?;
				let genesis_state = format!("0x{:?}", HexDisplay::from(&block.header().encode()));

				let task_executor = config.tokio_handle.clone();
				let polkadot_config =
					SubstrateCli::create_configuration(&polkadot_cli, &polkadot_cli, task_executor)
						.map_err(|err| format!("Relay chain argument error: {err}"))?;

				info!(
					"Relay-chain Chain spec: {:?}",
					polkadot_cli
						.shared_params()
						.chain_id(polkadot_cli.shared_params().is_dev())
				);
				info!("Parachain spec: {:?}", cli.run.base.shared_params.chain);
				info!("Parachain id: {:?}", id);
				info!("Parachain Account: {}", parachain_account);
				info!("Parachain genesis state: {}", genesis_state);
				info!(
					"Is collating: {}",
					if config.role.is_authority() {
						"yes"
					} else {
						"no"
					}
				);

				if !collator_options.relay_chain_rpc_urls.is_empty() && !cli.relaychain_args.is_empty() {
					warn!("Detected relay chain node arguments together with --relay-chain-rpc-urls. This command starts a minimal Polkadot node that only uses a network-related subset of all relay chain CLI options.");
				}

				match config.chain_spec.identify() {
					ChainIdentity::Altair => {
						crate::service::start_altair_node(config, polkadot_config, cli.eth, collator_options, id)
							.await
							.map(|r| r.0)
							.map_err(Into::into)
					}
					ChainIdentity::Centrifuge => crate::service::start_centrifuge_node(
						config,
						polkadot_config,
                        cli.eth,
						collator_options,
						id,
					)
					.await
					.map(|r| r.0)
					.map_err(Into::into),
					ChainIdentity::Development => crate::service::start_development_node(
						config,
						polkadot_config,
                        cli.eth,
						collator_options,
						id,
					)
					.await
					.map(|r| r.0)
					.map_err(Into::into),
				}
			})
		}
	}
}

impl DefaultConfigurationValues for RelayChainCli {
	fn p2p_listen_port() -> u16 {
		30334
	}

	fn rpc_ws_listen_port() -> u16 {
		9945
	}

	fn rpc_http_listen_port() -> u16 {
		9934
	}

	fn prometheus_listen_port() -> u16 {
		9616
	}
}

impl CliConfiguration<Self> for RelayChainCli {
	fn shared_params(&self) -> &SharedParams {
		self.base.base.shared_params()
	}

	fn import_params(&self) -> Option<&ImportParams> {
		self.base.base.import_params()
	}

	fn keystore_params(&self) -> Option<&KeystoreParams> {
		self.base.base.keystore_params()
	}

	fn network_params(&self) -> Option<&NetworkParams> {
		self.base.base.network_params()
	}

	fn base_path(&self) -> Result<Option<BasePath>> {
		Ok(self
			.shared_params()
			.base_path()
			.ok()
			.flatten()
			.or_else(|| self.base_path.clone().map(Into::into)))
	}

	fn role(&self, is_dev: bool) -> Result<sc_service::Role> {
		self.base.base.role(is_dev)
	}

	fn transaction_pool(&self, is_dev: bool) -> Result<sc_service::config::TransactionPoolOptions> {
		self.base.base.transaction_pool(is_dev)
	}

	fn chain_id(&self, is_dev: bool) -> Result<String> {
		self.base.base.chain_id(is_dev)
	}

	fn rpc_http(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
		self.base.base.rpc_http(default_listen_port)
	}

	fn rpc_ipc(&self) -> Result<Option<String>> {
		self.base.base.rpc_ipc()
	}

	fn rpc_ws(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
		self.base.base.rpc_ws(default_listen_port)
	}

	fn rpc_methods(&self) -> Result<sc_service::config::RpcMethods> {
		self.base.base.rpc_methods()
	}

	fn rpc_ws_max_connections(&self) -> Result<Option<usize>> {
		self.base.base.rpc_ws_max_connections()
	}

	fn rpc_cors(&self, is_dev: bool) -> Result<Option<Vec<String>>> {
		self.base.base.rpc_cors(is_dev)
	}

	fn prometheus_config(
		&self,
		default_listen_port: u16,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<PrometheusConfig>> {
		self.base
			.base
			.prometheus_config(default_listen_port, chain_spec)
	}

	fn telemetry_endpoints(
		&self,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<sc_telemetry::TelemetryEndpoints>> {
		self.base.base.telemetry_endpoints(chain_spec)
	}

	fn default_heap_pages(&self) -> Result<Option<u64>> {
		self.base.base.default_heap_pages()
	}

	fn force_authoring(&self) -> Result<bool> {
		self.base.base.force_authoring()
	}

	fn disable_grandpa(&self) -> Result<bool> {
		self.base.base.disable_grandpa()
	}

	fn max_runtime_instances(&self) -> Result<Option<usize>> {
		self.base.base.max_runtime_instances()
	}

	fn announce_block(&self) -> Result<bool> {
		self.base.base.announce_block()
	}

	fn init<F>(
		&self,
		_support_url: &String,
		_impl_version: &String,
		_logger_hook: F,
		_config: &sc_service::Configuration,
	) -> Result<()>
	where
		F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
	{
		unreachable!("PolkadotCli is never initialized; qed");
	}
}
