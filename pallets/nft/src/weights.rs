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


//! Non-fungible token (NFT) processing pallet's extrinsics weight information
//! 
//! Note that the following weights are used only for development.
//! In fact, weights shoudl be calculated using runtime benchmarking.

use frame_support::{
    weights::{
        constants::RocksDbWeight,
        Weight,
    }
};

use crate::traits::WeightInfo;


impl WeightInfo for () {
    
    fn transfer() -> Weight {
        (195_000_000 as Weight).saturating_add(RocksDbWeight::get().reads_writes(1,1))
    }
}