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

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unit_arg)]

///! Common-types of the Centrifuge chain.
pub mod adjustments;
pub mod consts;
pub mod epoch;
pub mod fee_keys;
pub mod fixed_point;
pub mod ids;
pub mod investments;
pub mod orders;
pub mod permissions;
pub mod time;
pub mod tokens;
pub mod xcm;
