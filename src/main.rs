//! Substrate Node Template CLI library.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

mod api;
mod chain_spec;
#[macro_use]
mod service;
mod cli;

pub use substrate_cli::{error, IntoExit, VersionInfo};

fn main() -> Result<(), cli::error::Error> {
    let version = VersionInfo {
        name: "Centrifuge Chain Node",
        commit: env!("VERGEN_SHA_SHORT"),
        version: env!("CARGO_PKG_VERSION"),
        executable_name: "centrifuge-chain",
        author: "Centrifuge",
        description: "Centrifuge Chain Node",
        support_url: "centrifuge.io",
    };

    cli::run(std::env::args(), cli::Exit, version)
}
