// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General

use sp_runtime::traits::CheckedAdd;

/// Advances the chain `n` number of blocks
/// for use with tests where actions would need to take
/// place a certain number of blocks after another action.
/// Note that this does not do finalization or initialisation of blocks.
pub fn advance_n_blocks<T: frame_system::Config>(n: <T as frame_system::Config>::BlockNumber) {
	let b = frame_system::Pallet::<T>::block_number()
		.checked_add(&n)
		.expect("Mock block advancement failed.");
	frame_system::Pallet::<T>::set_block_number(b)
}
