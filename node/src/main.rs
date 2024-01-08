//! Centrifuge Chain Node CLI library.
#![warn(missing_docs)]
// Allow things like `1 * CFG`
#![allow(clippy::identity_op)]

mod chain_spec;
mod cli;
mod command;
mod data_extension_worker;
mod rpc;
#[macro_use]
mod service;

fn main() -> sc_cli::Result<()> {
	command::run()
}
