#![feature(stmt_expr_attributes)]
// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
#![cfg(test)]
#![allow(unused)]

mod pools;
mod runtime_apis;
mod xcm;

/// Re-exports the correct runtimes that we run the integration tests with
/// This allows all other modules to use the import of `crate::chain::{...}`
/// in order to get the right stuff from the respective runtime.
mod chain {
	pub mod centrifuge {
		#[cfg(feature = "runtime-altair")]
		pub use altair::*;
		#[cfg(feature = "runtime-centrifuge")]
		pub use centrifuge::*;
		#[cfg(feature = "runtime-development")]
		pub use development::*;

		#[cfg(feature = "runtime-centrifuge")]
		pub mod centrifuge {
			pub use centrifuge_runtime::*;
			pub const PARA_ID: u32 = 2031;
		}

		#[cfg(feature = "runtime-altair")]
		pub mod altair {
			pub use altair_runtime::*;
			pub const PARA_ID: u32 = 2088;
		}

		#[cfg(feature = "runtime-development")]
		pub mod development {
			pub use development_runtime::*;
			pub const PARA_ID: u32 = 2000;
		}
	}

	pub mod relay {
		#[cfg(feature = "runtime-altair")]
		pub use kusama_runtime::*;
		#[cfg(feature = "runtime-centrifuge")]
		pub use polkadot_runtime::*;
		#[cfg(feature = "runtime-development")]
		pub use rococo_runtime::*;
	}
}
