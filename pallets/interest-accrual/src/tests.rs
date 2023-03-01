// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use sp_runtime::{
	traits::{One, Zero},
	FixedPointNumber,
};

use crate::{
	mock::{Rate, Runtime},
	Pallet,
};

#[test]
fn test_rate_validation() {
	let high_rate = Rate::saturating_from_rational(300000, 10000);
	let min_rate = Rate::saturating_from_rational(1, 10000);
	let normal_rate = Rate::saturating_from_rational(5, 100);
	let too_many_decimals = Rate::saturating_from_rational(55, 100000);

	assert!(Pallet::<Runtime>::validate_rate(high_rate).is_ok());
	assert!(Pallet::<Runtime>::validate_rate(min_rate).is_ok());
	assert!(Pallet::<Runtime>::validate_rate(normal_rate).is_ok());
	assert!(Pallet::<Runtime>::validate_rate(One::one()).is_ok());
	assert!(Pallet::<Runtime>::validate_rate(Zero::zero()).is_ok());
	assert!(Pallet::<Runtime>::validate_rate(too_many_decimals).is_err());
}
