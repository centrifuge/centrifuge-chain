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

use cfg_traits::Permissions;
use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{dispatch::RawOrigin, traits::EnsureOriginWithArg};

pub struct LiquidityAndPoolAdminOrRoot<T>(sp_std::marker::PhantomData<T>);

impl<
		T: frame_system::Config
			+ pallet_permissions::Config<
				Scope = PermissionScope<T::PoolId, T::CurrencyId>,
				Role = Role<T::TrancheId>,
			> + pallet_pool_system::Config,
	> EnsureOriginWithArg<T::RuntimeOrigin, T::PoolId> for LiquidityAndPoolAdminOrRoot<T>
where
	<T as frame_system::Config>::RuntimeOrigin: From<RawOrigin<<T as frame_system::Config>::AccountId>>
		+ Into<Result<RawOrigin<<T as frame_system::Config>::AccountId>, T::RuntimeOrigin>>,
{
	type Success = ();

	fn try_origin(
		o: T::RuntimeOrigin,
		pool_id: &T::PoolId,
	) -> Result<Self::Success, T::RuntimeOrigin> {
		o.into().and_then(|r| match r {
			RawOrigin::Root => Ok(()),
			RawOrigin::Signed(by) => {
				if <pallet_permissions::Pallet<T> as Permissions<T::AccountId>>::has(
					PermissionScope::Pool(*pool_id),
					by.clone(),
					Role::PoolRole(PoolRole::PoolAdmin),
				) || <pallet_permissions::Pallet<T> as Permissions<T::AccountId>>::has(
					PermissionScope::Pool(*pool_id),
					by.clone(),
					Role::PoolRole(PoolRole::LiquidityAdmin),
				) {
					Ok(())
				} else {
					Err(T::RuntimeOrigin::from(RawOrigin::Signed(by)))
				}
			}
			RawOrigin::None => Err(T::RuntimeOrigin::from(RawOrigin::None)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(_: &T::PoolId) -> Result<T::RuntimeOrigin, ()> {
		Ok(T::RuntimeOrigin::root())
	}
}
