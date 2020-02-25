//! Substrate Node CLI library.
#![warn(missing_docs)]

mod api;
mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod command;

pub use sc_cli::{VersionInfo, error};

fn main() -> Result<(), error::Error> {
	let version = VersionInfo {
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
