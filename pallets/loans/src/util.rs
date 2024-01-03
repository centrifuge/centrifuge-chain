// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::changes::ChangeGuard;
use sp_runtime::DispatchError;
use sp_std::marker::PhantomData;

use crate::{entities::changes::Change, pallet::Config};

const DEFAULT_CHANGE_ERR: DispatchError =
	DispatchError::Other("No configured change system for pallet-loans");

/// Type used to configure the pallet without changes support
pub struct NoLoanChanges<T>(PhantomData<T>);

impl<T: Config> ChangeGuard for NoLoanChanges<T> {
	type Change = Change<T>;
	type ChangeId = T::Hash;
	type PoolId = T::PoolId;

	fn note(_: T::PoolId, _: Self::Change) -> Result<Self::ChangeId, DispatchError> {
		Err(DEFAULT_CHANGE_ERR)
	}

	fn released(_: T::PoolId, _: Self::ChangeId) -> Result<Self::Change, DispatchError> {
		Err(DEFAULT_CHANGE_ERR)
	}
}
