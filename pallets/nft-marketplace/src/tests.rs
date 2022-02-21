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

use crate::mock::*;
use common_types::CurrencyId;
use frame_support::dispatch::DispatchError;
use frame_support::{assert_noop};
use runtime_common::InstanceId;

/// Verify that calling `NftMarketplace::add` specifiying an nft that is not present in the
/// underlying `pallet_uniques` fails with `nft_marketplace::Error::<T>::NotFound`.
#[test]
fn nft_not_found() {
	new_test_ext().execute_with(|| {
		let origin: Origin = Origin::signed(33);
		let unknown_asset = (0, InstanceId(1));

		assert_noop!(
			NftMarketplace::add(origin, unknown_asset.0, unknown_asset.1, CurrencyId::Usd, 3),
			DispatchError::from(nft_marketplace::Error::<Test>::NotFound)
		);
	});
}
