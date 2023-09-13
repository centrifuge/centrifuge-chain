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

use codec::{Decode, Encode, Input};
use sp_std::{cmp::min, vec::Vec};

/// Build a fixed-size array using as many elements from `src` as possible
/// without overflowing and ensuring that the array is 0 padded in the case
/// where `src.len()` is smaller than S.
pub fn vec_to_fixed_array<const S: usize>(src: Vec<u8>) -> [u8; S] {
	let mut dest = [0; S];
	let len = min(src.len(), S);
	dest[..len].copy_from_slice(&src.as_slice()[..len]);

	dest
}

/// Encode a value in its big-endian representation of which all we know is that
/// it implements Encode. We use this for number types to make sure they are
/// encoded the way they are expected to be decoded on the Solidity side.
pub fn encode_be(x: impl Encode) -> Vec<u8> {
	let mut output = x.encode();
	output.reverse();
	output
}

/// Decode a type O by reading S bytes from I. Those bytes are expected to be
/// encoded as big-endian and thus needs reversing to little-endian before
/// decoding to O.
pub fn decode_be_bytes<const S: usize, O: Decode, I: Input>(
	input: &mut I,
) -> Result<O, codec::Error> {
	let mut bytes = [0; S];
	input.read(&mut bytes[..])?;
	bytes.reverse();

	O::decode(&mut bytes.as_slice())
}

/// Decode a type 0 by reading S bytes from I.
pub fn decode<const S: usize, O: Decode, I: Input>(input: &mut I) -> Result<O, codec::Error> {
	let mut bytes = [0; S];
	input.read(&mut bytes[..])?;

	O::decode(&mut bytes.as_slice())
}

/// Function that initializes the frame system & Aura, so a timestamp can be set
/// and pass validation
#[cfg(any(feature = "runtime-benchmarks", feature = "std"))]
pub fn set_block_number_timestamp<T>(block_number: T::BlockNumber, timestamp: T::Moment)
where
	T: pallet_aura::Config + frame_system::Config + pallet_timestamp::Config,
{
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

pub fn decode_var_source<const EXPECTED_SOURCE_ADDRESS_SIZE: usize>(
	source_address: &[u8],
) -> Option<[u8; EXPECTED_SOURCE_ADDRESS_SIZE]> {
	const HEX_PREFIX: &str = "0x";

	let mut address = [0u8; EXPECTED_SOURCE_ADDRESS_SIZE];

	if source_address.len() == EXPECTED_SOURCE_ADDRESS_SIZE {
		address.copy_from_slice(source_address);
		return Some(address);
	}

	let try_bytes = match sp_std::str::from_utf8(source_address) {
		Ok(res) => res.as_bytes(),
		Err(_) => source_address,
	};

	// Attempt to hex decode source address.
	let bytes = match hex::decode(try_bytes) {
		Ok(res) => Some(res),
		Err(_) => {
			// Strip 0x prefix.
			let res = try_bytes.strip_prefix(HEX_PREFIX.as_bytes())?;

			hex::decode(res).ok()
		}
	}?;

	if bytes.len() == EXPECTED_SOURCE_ADDRESS_SIZE {
		address.copy_from_slice(bytes.as_slice());
		Some(address)
	} else {
		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	mod get_source_address_bytes {
		const EXPECTED: usize = 20;
		use super::*;

		#[test]
		fn get_source_address_bytes_works() {
			let hash = [1u8; 20];

			decode_var_source::<EXPECTED>(&hash).expect("address bytes from H160 works");

			let str = String::from("d47ed02acbbb66ee8a3fe0275bd98add0aa607c3");

			decode_var_source::<EXPECTED>(str.as_bytes())
				.expect("address bytes from un-prefixed hex works");

			let str = String::from("0xd47ed02acbbb66ee8a3fe0275bd98add0aa607c3");

			decode_var_source::<EXPECTED>(str.as_bytes())
				.expect("address bytes from prefixed hex works");
		}
	}

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
		// the desired output fixed-array by copying all elements of source to said
		// array.
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
		// desired output fixed-array by copying all elements of source until said array
		// is full.
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
