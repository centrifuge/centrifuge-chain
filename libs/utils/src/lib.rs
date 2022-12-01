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

use sp_std::{cmp::min, vec::Vec};

/// Build a fixed-size array using as many elements from `src` as possible without
/// overflowing and ensuring that the array is 0 padded in the case where
/// `src.len()` is smaller than S.
pub fn vec_to_fixed_array<const S: usize>(src: Vec<u8>) -> [u8; S] {
	let mut dest = [0; S];
	let len = min(src.len(), S);
	dest[..len].copy_from_slice(&src.as_slice()[..len]);

	dest
}

/// Function that initializes the frame system & Aura, so a timestamp can be set and pass validation
#[cfg(any(feature = "runtime-benchmarks", feature = "std"))]
pub fn set_block_number_timestamp<T>(block_number: T::BlockNumber, timestamp: T::Moment)
where
	T: pallet_aura::Config + frame_system::Config + pallet_timestamp::Config,
{
	use codec::Encode;
	use frame_support::traits::Hooks;
	use sp_consensus_aura::AURA_ENGINE_ID;
	use sp_runtime::{Digest, DigestItem};
	use sp_std::vec;

	let slot = timestamp / pallet_aura::Pallet::<T>::slot_duration();
	let digest = Digest {
		logs: vec![DigestItem::PreRuntime(AURA_ENGINE_ID, slot.encode())],
	};
	frame_system::Pallet::<T>::initialize(&block_number, &Default::default(), &digest);
	pallet_aura::Pallet::<T>::on_initialize(block_number);
	pallet_timestamp::Pallet::<T>::set_timestamp(timestamp);
}

#[cfg(test)]
mod tests {
	use super::*;

	mod vec_to_fixed_array {
		use super::*;

		// Verify that `vec_to_fixed_array` converts a source Vec that's shorter than
		// the desired output fixed-array by copying all elements of source and filling
		// the remaining bytes to 0.
		#[test]
		fn short_source() {
			let src = "TrNcH".as_bytes().to_vec();
			let symbol: [u8; 32] = vec_to_fixed_array(src.clone());

			assert!(symbol.starts_with("TrNcH".as_bytes()));
			assert_eq!(
				symbol,
				[
					84, 114, 78, 99, 72, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
					0, 0, 0, 0, 0, 0, 0, 0
				]
			);
		}

		// Verify that `vec_to_fixed_array` converts a source Vec that's exactly as big
		// the desired output fixed-array by copying all elements of source to said array.
		#[test]
		fn max_source() {
			let src: Vec<u8> = (0..32).collect();
			let symbol: [u8; 32] = vec_to_fixed_array(src.clone());

			assert_eq!(
				symbol,
				[
					0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
					22, 23, 24, 25, 26, 27, 28, 29, 30, 31
				]
			);
		}

		// Verify that `vec_to_fixed_array` converts a source Vec that's longer than the
		// desired output fixed-array by copying all elements of source until said array is full.
		#[test]
		fn exceeding_source() {
			let src: Vec<u8> = (0..64).collect();
			let symbol: [u8; 32] = vec_to_fixed_array(src.clone());

			assert_eq!(
				symbol,
				[
					0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
					22, 23, 24, 25, 26, 27, 28, 29, 30, 31
				]
			);
		}
	}
}
