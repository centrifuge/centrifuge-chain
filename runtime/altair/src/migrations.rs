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
use cfg_primitives::Balance;
use cfg_types::tokens::CurrencyId;
use codec::{Decode, Encode};
#[cfg(feature = "try-runtime")]
use frame_support::ensure;
use frame_support::{
	traits::OnRuntimeUpgrade,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::vec::Vec;

use crate::Runtime;

pub type UpgradeAltair1029 =
	pallet_loans::migrations::nuke::Migration<crate::Loans, RocksDbWeight, 2>;
