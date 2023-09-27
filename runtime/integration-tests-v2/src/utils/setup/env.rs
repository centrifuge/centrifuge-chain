// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use fudge::{ParachainBuilder, RelaychainBuilder};

use crate::utils::setup::chains::{centrifuge, relay};

/// A struct that stores all events that have been generated
#[fudge::companion]
pub struct TestEnv {
	#[fudge::relaychain]
	pub relay:
		RelaychainBuilder<relay::Block, relay::RuntimeApi, relay::Runtime, relay::Cidp, relay::Dp>,
	#[fudge::parachain(centrifuge::PARA_ID)]
	pub centrifuge: ParachainBuilder<
		centrifuge::Block,
		centrifuge::RuntimeApi,
		centrifuge::Cidp,
		centrifuge::Dp,
	>,
}
