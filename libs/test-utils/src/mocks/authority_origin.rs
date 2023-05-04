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
use cfg_types::tokens::CurrencyId;
use frame_support::traits::{EnsureOrigin, EnsureOriginWithArg};
use frame_system::RawOrigin;
use sp_std::marker::PhantomData;

type AccountId = u64;

/// This OrmlAssetRegistry::AuthorityOrigin implementation is used for our
/// pallet-loans and pallet-pool-system Mocks. We overwrite this because of the
/// `type AccountId = u64`. In the runtime tests, we use proper AccountIds, in
/// the Mocks, we use 1,2,3,... . Therefore, we implement `AuthorityOrigin` and
/// use the `u64` type for the AccountId.
///
/// Use this implementation only when setting up Mocks with simple AccountIds.
pub struct AuthorityOrigin<
	// The origin type
	Origin,
	// The default EnsureOrigin impl used to authorize all
	// assets besides tranche tokens.
	DefaultEnsureOrigin,
>(PhantomData<(Origin, DefaultEnsureOrigin)>);

impl<
		Origin: Into<Result<RawOrigin<AccountId>, Origin>> + From<RawOrigin<AccountId>>,
		EnsureRoot: EnsureOrigin<Origin>,
	> EnsureOriginWithArg<Origin, Option<CurrencyId>> for AuthorityOrigin<Origin, EnsureRoot>
{
	type Success = ();

	fn try_origin(origin: Origin, asset_id: &Option<CurrencyId>) -> Result<Self::Success, Origin> {
		match asset_id {
			// Only the pools pallet should directly register/update tranche tokens
			Some(CurrencyId::Tranche(_, _)) => Err(origin),

			// Any other `asset_id` defaults to EnsureRoot
			_ => EnsureRoot::try_origin(origin).map(|_| ()),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin(_asset_id: &Option<CurrencyId>) -> Origin {
		todo!()
	}
}
