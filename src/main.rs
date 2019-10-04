//! Substrate Node Template CLI library.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

mod api;
mod chain_spec;
mod cli;
mod service;

pub use substrate_cli::{error, IntoExit, VersionInfo};

fn main() {
    let version = VersionInfo {
        name: "Centrifuge Chain Node",
        commit: env!("VERGEN_SHA_SHORT"),
        version: env!("CARGO_PKG_VERSION"),
        executable_name: "centrifuge-chain",
        author: "Centrifuge",
        description: "Centrifuge Chain Node",
        support_url: "centrifuge.io",
    };

    if let Err(e) = cli::run(::std::env::args(), cli::Exit, version) {
        eprintln!("Fatal error: {}\n\n{:?}", e, e);
        std::process::exit(1)
    }
}
