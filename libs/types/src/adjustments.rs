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

use sp_runtime::{
	traits::{EnsureAdd, EnsureSub, Zero},
	ArithmeticError,
};

#[derive(Clone, Copy)]
pub enum Adjustment<Amount> {
	Increase(Amount),
	Decrease(Amount),
}

impl<Amount> Adjustment<Amount> {
	pub fn abs(self) -> Amount {
		match self {
			Adjustment::Increase(amount) => amount,
			Adjustment::Decrease(amount) => amount,
		}
	}

	pub fn map<F, R>(self, f: F) -> Adjustment<R>
	where
		F: FnOnce(Amount) -> R,
	{
		match self {
			Adjustment::Increase(amount) => Adjustment::Increase(f(amount)),
			Adjustment::Decrease(amount) => Adjustment::Decrease(f(amount)),
		}
	}

	pub fn try_map<F, E, R>(self, f: F) -> Result<Adjustment<R>, E>
	where
		F: FnOnce(Amount) -> Result<R, E>,
	{
		match self {
			Adjustment::Increase(amount) => f(amount).map(Adjustment::Increase),
			Adjustment::Decrease(amount) => f(amount).map(Adjustment::Decrease),
		}
	}
}

impl<Amount: EnsureAdd + EnsureSub> Adjustment<Amount> {
	pub fn ensure_add(self, amount: Amount) -> Result<Amount, ArithmeticError> {
		match self {
			Adjustment::Increase(inc) => amount.ensure_add(inc),
			Adjustment::Decrease(dec) => amount.ensure_sub(dec),
		}
	}
}
