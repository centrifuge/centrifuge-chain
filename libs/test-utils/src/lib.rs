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

//! Test utilities that can be used across multiple pallets.
//! Providing all sorts of mock implementations for traits the pallets
//! need and which can be used for mock environments

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::PalletId;

pub mod mocks;
pub mod system;

/// A pallet id for testing.
/// Can be used for a single point of storage for tokens in testing for example
pub const TEST_PALLET_ID: PalletId = PalletId(*b"TestTest");
