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

use sp_runtime::traits::{CheckedAdd, Header, One};

pub fn advance_n_blocks<T: frame_system::Config>(n: u64) {
	for _ in 0..n {
		let h = frame_system::Pallet::<T>::finalize();
		let b = h
			.number()
			.checked_add(&T::BlockNumber::one())
			.expect("Mock block count increase failed");
		frame_system::Pallet::<T>::initialize(&b, h.parent_hash(), h.digest());
	}
}
