//! Centrifuge Chain Node CLI library.
#![warn(missing_docs)]

mod api;
mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod command;

fn main() -> sc_cli::Result<()> {
	let version = sc_cli::VersionInfo {
		name: "Centrifuge Chain Node",
		commit: env!("VERGEN_SHA_SHORT"),
		version: env!("CARGO_PKG_VERSION"),
		executable_name: "centrifuge-chain",
		author: "Centrifuge",
		description: "Centrifuge Chain Node",
		support_url: "centrifuge.io",
		copyright_start_year: 2019,
	};

	command::run(version)
}
