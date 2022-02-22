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
use frame_support::{assert_noop, assert_ok};
use runtime_common::{InstanceId};
use frame_support::traits::fungibles::Inspect;

/// Verify that calling `NftMarketplace::add` specifiying an nft that is not present in the
/// underlying `pallet_uniques` fails with `nft_marketplace::Error::<T>::NotFound`.
#[test]
fn add_nft_not_found() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(33);
		let unknown_asset = (0, InstanceId(1));

		assert_noop!(
			NftMarketplace::add(seller, unknown_asset.0, unknown_asset.1, CurrencyId::Usd, 3),
			DispatchError::from(nft_marketplace::Error::<Test>::NotFound)
		);
	});
}

#[test]
fn remove_nft_not_found() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(33);
		let unknown_asset = (0, InstanceId(1));

		assert_noop!(
			NftMarketplace::remove(seller, unknown_asset.0, unknown_asset.1),
			DispatchError::from(nft_marketplace::Error::<Test>::NotFound)
		);
	});
}

#[test]
fn buy_nft_not_found() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(33);
		let unknown_asset = (0, InstanceId(1));

		assert_noop!(
			NftMarketplace::buy(seller, unknown_asset.0, unknown_asset.1),
			DispatchError::from(nft_marketplace::Error::<Test>::NotFound)
		);
	});
}

#[test]
fn add_nft_works() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(1);
		let (class_id, instance_id) = (0, InstanceId(1));

		// Mint the nft in the uniques pallet
		assert_ok!(Uniques::create(seller.clone(), class_id, 1));
		assert_ok!(Uniques::mint(seller.clone(), class_id, instance_id, 1));

		// Set it for sale in the NftMarketplace
		assert_ok!(NftMarketplace::add(
			seller.clone(),
			class_id,
			instance_id,
			CurrencyId::Usd,
			10_000
		));

		// Verify that if the seller tries to put it for sale again, that it fails with `AlreadyForSale`
		assert_noop!(
			NftMarketplace::add(seller, class_id, instance_id, CurrencyId::Usd, 10_000),
			DispatchError::from(nft_marketplace::Error::<Test>::AlreadyForSale)
		);
	});
}

#[test]
fn remove_nft_works() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(1);
		let (class_id, instance_id) = (0, InstanceId(1));

		// Mint the nft in the uniques pallet
		assert_ok!(Uniques::create(seller.clone(), class_id, 1));
		assert_ok!(Uniques::mint(seller.clone(), class_id, instance_id, 1));

		// Set it for sale in the NftMarketplace
		assert_ok!(NftMarketplace::add(
			seller.clone(),
			class_id,
			instance_id,
			CurrencyId::Usd,
			10_000
		));

		assert_ok!(
			NftMarketplace::remove(seller.clone(), class_id, instance_id),
		);

		// Verify that try and remove it again fails with `NotForSale`
		assert_noop!(
			NftMarketplace::remove(seller, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::NotForSale)
		);
	});
}


#[test]
fn buy_nft_fails_already_owner() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(1);
		let (class_id, instance_id) = (0, InstanceId(1));

		// Mint the nft in the uniques pallet
		assert_ok!(Uniques::create(seller.clone(), class_id, 1));
		assert_ok!(Uniques::mint(seller.clone(), class_id, instance_id, 1));

		// Set it for sale in the NftMarketplace
		assert_ok!(NftMarketplace::add(
			seller.clone(),
			class_id,
			instance_id,
			CurrencyId::Usd,
			10_000
		));

		// Verify that the seller cannot buy the item they already own
		assert_noop!(
			NftMarketplace::buy(seller, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::AlreadyOwner)
		);
	});
}

#[test]
fn buy_nft_fails_not_for_sale() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(1);
		let (class_id, instance_id) = (0, InstanceId(1));

		// Mint the nft in the uniques pallet
		assert_ok!(Uniques::create(seller.clone(), class_id, 1));
		assert_ok!(Uniques::mint(seller, class_id, instance_id, 1));

		// Verify that the buyer cannot buy said asset because it's not for sale
		let buyer: Origin = Origin::signed(2);
		assert_noop!(
			NftMarketplace::buy(buyer, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::NotForSale)
		);
	});
}

#[test]
fn buy_nft_fails_insufficient_balance() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(1);
		let (class_id, instance_id) = (0, InstanceId(1));

		// Mint the nft in the uniques pallet
		assert_ok!(Uniques::create(seller.clone(), class_id, 1));
		assert_ok!(Uniques::mint(seller.clone(), class_id, instance_id, 1));

		// Set it for sale in the NftMarketplace
		assert_ok!(NftMarketplace::add(
			seller.clone(),
			class_id,
			instance_id,
			CurrencyId::Usd,
			OrmlTokens::balance(CurrencyId::Usd, &1) + 1
		));

		// Verify that the buyer cannot buy said asset because its asking price
		// exceeds the seller's balance.
		let buyer: Origin = Origin::signed(2);
		assert_noop!(
			NftMarketplace::buy(buyer, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::InsufficientBalance)
		);
	});
}


#[test]
fn buy_nft_works() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(1);
		let (class_id, instance_id) = (0, InstanceId(1));

		// Mint the nft in the uniques pallet
		assert_ok!(Uniques::create(seller.clone(), class_id, 1));
		assert_ok!(Uniques::mint(seller.clone(), class_id, instance_id, 1));

		// Set it for sale in the NftMarketplace
		assert_ok!(NftMarketplace::add(
			seller.clone(),
			class_id,
			instance_id,
			CurrencyId::Usd,
			10_000
		));

		// Verify that the buyer can buy the nft
		let buyer: Origin = Origin::signed(2);
		assert_ok!(NftMarketplace::buy(buyer, class_id, instance_id));

		// TODO(nuno): Verify other things, namely:
		// - we are no longer the freezer / the asset is no longer frozen
		// - the buyer is now the owner of the freezer
		// - the asking price was deducted appropriately
	});
}