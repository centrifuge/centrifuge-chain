// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Common types, traits and constants used by the Centrifuge Chain
//!
//! ## Overview
//! This crate implements shared types, constants and traits.
//!
//! ## Credits
//! The Centrifugians Tribe <tribe@centrifuge.io>
//!
//! ## License
//! GNU General Public License, Version 3, 29 June 2007 <https://www.gnu.org/licenses/gpl-3.0.html>

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

// Common types and traits definition
pub mod traits;
pub mod types;

// ----------------------------------------------------------------------------
// Constants definition
// ----------------------------------------------------------------------------

pub mod constants {
	/// Represents the protobuf encoding - "NFTS". All Centrifuge documents are formatted in this way.
	/// These are pre/appended to the registry id before being set as a [RegistryInfo] field in [create_registry].
	pub const NFTS_PREFIX: &'static [u8] = &[1, 0, 0, 0, 0, 0, 0, 20];
	pub const MS_PER_DAY: u64 = 86400000;
}
