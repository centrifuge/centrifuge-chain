//! Centrifuge Chain Node CLI library.
#![warn(missing_docs)]

mod api;
mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod command;
mod child;

fn main() -> sc_cli::Result<()> {
	command::run()
}
