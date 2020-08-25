use sc_cli::{RunCmd, Subcommand};
use structopt::StructOpt;

/// An overarching CLI command definition.
#[derive(Debug, StructOpt)]
pub struct Cli {
	/// Possible subcommand with parameters.
	#[structopt(subcommand)]
	pub subcommand: Option<Subcommand>,
	#[allow(missing_docs)]
	#[structopt(flatten)]
	pub run: RunCmd,
}
