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

use parity_scale_codec::Encode;
use sp_std::cmp::min;

pub struct BufferReader<'a>(pub &'a [u8]);

impl<'a> BufferReader<'a> {
	pub fn read_bytes(&mut self, bytes: usize) -> Option<&[u8]> {
		if self.0.len() < bytes {
			return None;
		}

		let (consumed, remaining) = self.0.split_at(bytes);
		self.0 = remaining;
		Some(consumed)
	}

	pub fn read_array<const N: usize>(&mut self) -> Option<&[u8; N]> {
		let (consumed, remaining) = self.0.split_first_chunk::<N>()?;
		self.0 = remaining;
		Some(consumed)
	}
}

/// Build a fixed-size array using as many elements from `src` as possible
/// without overflowing and ensuring that the array is 0 padded in the case
/// where `src.len()` is smaller than S.
pub fn vec_to_fixed_array<const S: usize>(src: impl AsRef<[u8]>) -> [u8; S] {
	let mut dest = [0; S];
	let len = min(src.as_ref().len(), S);
	dest[..len].copy_from_slice(&src.as_ref()[..len]);

	dest
}

/// Function that initializes the frame system & Aura, so a timestamp can be set
/// and pass validation
#[cfg(any(feature = "runtime-benchmarks", feature = "std"))]
pub fn set_block_number_timestamp<T>(
	block_number: frame_system::pallet_prelude::BlockNumberFor<T>,
	timestamp: T::Moment,
) where
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

pub mod math {
	use sp_arithmetic::{
		traits::{BaseArithmetic, EnsureFixedPointNumber},
		ArithmeticError, FixedPointOperand, FixedU128,
	};

	/// Returns the coordinate `y` for coordinate `x`,
	/// in a function given by 2 points: (x1, y1) and (x2, y2)
	pub fn y_coord_in_rect<X, Y>(
		(x1, y1): (X, Y),
		(x2, y2): (X, Y),
		x: X,
	) -> Result<Y, ArithmeticError>
	where
		X: BaseArithmetic + FixedPointOperand,
		Y: BaseArithmetic + FixedPointOperand,
	{
		// From the equation: (x - x1) / (x2 - x1) == (y - y1) / (y2 - y1) we solve y:
		//
		// NOTE: With rects that have x or y negative directions, we emulate a
		// symmetry in those axis to avoid unsigned underflows in substractions. It
		// means, we first "convert" the rect into an increasing rect, and in such rect,
		// we find the y coordinate.

		let left = if x1 <= x2 {
			FixedU128::ensure_from_rational(x.ensure_sub(x1)?, x2.ensure_sub(x1)?)?
		} else {
			// X symmetry emulation
			FixedU128::ensure_from_rational(x1.ensure_sub(x)?, x1.ensure_sub(x2)?)?
		};

		if y1 <= y2 {
			left.ensure_mul_int(y2.ensure_sub(y1)?)?.ensure_add(y1)
		} else {
			// Y symmetry emulation
			y1.ensure_sub(left.ensure_mul_int(y1.ensure_sub(y2)?)?)
		}
	}

	/// Converts the given number to percent.
	///
	/// # Example
	///
	/// ```
	/// use sp_arithmetic::FixedI64;
	/// use cfg_utils::math::to_percent;
	///
	/// assert_eq!(to_percent(3u128), FixedI64::from_rational(3, 100));
	/// ```
	pub const fn to_percent(x: u128) -> sp_arithmetic::FixedI64 {
		sp_arithmetic::FixedI64::from_rational(x, 100)
	}

	/// Converts the given number to parts per million
	///
	/// # Example
	///
	/// ```
	/// use sp_arithmetic::FixedI64;
	/// use cfg_utils::math::to_ppm;
	///
	/// assert_eq!(to_ppm(3u128), FixedI64::from_rational(3, 1_000_000));
	/// ```
	pub const fn to_ppm(x: u128) -> sp_arithmetic::FixedI64 {
		sp_arithmetic::FixedI64::from_rational(x, 1_000_000)
	}

	#[cfg(test)]
	mod test_y_coord_in_function_with_2_points {
		use super::*;

		#[test]
		fn start_point() {
			assert_eq!(y_coord_in_rect::<u32, u32>((3, 12), (7, 24), 3), Ok(12));
		}

		#[test]
		fn end_point() {
			assert_eq!(y_coord_in_rect::<u32, u32>((3, 12), (7, 24), 7), Ok(24));
		}

		// Rect defined as:
		//    (x2, y2)
		//      /
		//     /
		// (x1, y1)
		#[test]
		fn inner_point() {
			assert_eq!(y_coord_in_rect::<u32, u32>((3, 12), (7, 24), 4), Ok(15));
		}

		// Rect defined as:
		// (x2, y2)
		//      \
		//       \
		//     (x1, y1)
		#[test]
		fn inner_point_with_greater_x1() {
			assert_eq!(y_coord_in_rect::<u32, u32>((7, 12), (3, 24), 4), Ok(21));
		}

		// Rect defined as:
		// (x1, y1)
		//      \
		//       \
		//     (x2, y2)
		#[test]
		fn inner_point_with_greater_y1() {
			assert_eq!(y_coord_in_rect::<u32, u32>((3, 24), (7, 12), 4), Ok(21));
		}

		// Rect defined as:
		//    (x1, y1)
		//      /
		//     /
		// (x2, y2)
		#[test]
		fn inner_point_with_greater_x1y1() {
			assert_eq!(y_coord_in_rect::<u32, u32>((7, 24), (3, 12), 4), Ok(15));
		}
	}
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
