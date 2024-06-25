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

pub mod accounts;
pub mod logs;

pub mod orml_asset_registry {
	// orml_asset_registry has remove the reexport of all pallet stuff,
	// we reexport it again here
	pub use orml_asset_registry::module::*;
}

pub mod approx {
	use std::fmt;

	use cfg_primitives::Balance;

	#[derive(Clone)]
	pub struct Approximation {
		value: Balance,
		offset: Balance,
		is_positive: bool,
	}

	impl PartialEq<Approximation> for Balance {
		fn eq(&self, ap: &Approximation) -> bool {
			match ap.is_positive {
				true => *self <= ap.value && *self + ap.offset >= ap.value,
				false => *self >= ap.value && *self - ap.offset <= ap.value,
			}
		}
	}

	impl fmt::Debug for Approximation {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			let (from, to) = match self.is_positive {
				true => (self.value - self.offset, self.value),
				false => (self.value, self.value + self.offset),
			};

			write!(f, "Approximation: [{}, {}]", from, to)
		}
	}

	/// Allow to compare `Balance` values with approximated values:
	pub trait Approximate {
		fn approx(&self, variation: f64) -> Approximation;
	}

	impl Approximate for Balance {
		fn approx(&self, variation: f64) -> Approximation {
			let offset = match variation >= 0.0 {
				true => (*self as f64 * variation) as Balance,
				false => (*self as f64 * -variation) as Balance,
			};

			Approximation {
				value: *self,
				offset,
				is_positive: variation >= 0.0,
			}
		}
	}

	#[test]
	fn approximations() {
		assert_eq!(1000u128, 996.approx(-0.01));
		assert_eq!(1000u128, 1004.approx(0.01));
		assert_eq!(1000u128, 1500.approx(0.5));

		assert_ne!(1000u128, 996.approx(0.01));
		assert_ne!(1000u128, 1004.approx(-0.01));
		assert_ne!(1000u128, 1500.approx(0.1));
	}
}
