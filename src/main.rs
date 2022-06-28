//! Centrifuge Chain Node CLI library.
#![warn(missing_docs)]

mod chain_spec;
mod cli;
mod command;
mod rpc;
#[macro_use]
mod service;

fn main() -> sc_cli::Result<()> {
	command::run()
}
