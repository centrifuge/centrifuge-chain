//! Centrifuge Chain Node CLI library.
#![warn(missing_docs)]

mod api;
mod chain_spec;
mod rpc;
#[macro_use]
mod service;
mod cli;
mod command;

fn main() -> sc_cli::Result<()> {
	command::run()
}
