//! Substrate Node Template CLI library.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

use futures::channel::oneshot;
use futures::{future, FutureExt};
use std::cell::RefCell;

pub use sc_cli::{VersionInfo, IntoExit, error};

mod api;
mod chain_spec;
mod executor;
#[macro_use]
mod service;
mod cli;

// handles ctrl-c
struct Exit;
impl sc_cli::IntoExit for Exit {
	type Exit = future::Map<oneshot::Receiver<()>, fn(Result<(), oneshot::Canceled>) -> ()>;
	fn into_exit(self) -> Self::Exit {
		// can't use signal directly here because CtrlC takes only `Fn`.
		let (exit_send, exit) = oneshot::channel();

		let exit_send_cell = RefCell::new(Some(exit_send));
		ctrlc::set_handler(move || {
			if let Some(exit_send) = exit_send_cell.try_borrow_mut().expect("signal handler not reentrant; qed").take() {
				exit_send.send(()).expect("Error sending exit notification");
			}
		}).expect("Error setting Ctrl-C handler");

		exit.map(|_| ())
	}
}

fn main() -> Result<(), sc_cli::error::Error> {
	let version = VersionInfo {
        name: "Centrifuge Chain Node",
		commit: env!("VERGEN_SHA_SHORT"),
        version: env!("CARGO_PKG_VERSION"),
        executable_name: "centrifuge-chain",
        author: "Centrifuge",
        description: "Centrifuge Chain Node",
        support_url: "centrifuge.io",
	};

	cli::run(std::env::args(), Exit, version)
}
