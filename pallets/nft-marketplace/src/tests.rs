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
use frame_support::traits::fungibles::Inspect;
use frame_support::{assert_noop, assert_ok};
use runtime_common::InstanceId;

/// Verify that calling `NftMarketplace::add` specifiying an nft that is not present in the
/// underlying `pallet_uniques` fails with `nft_marketplace::Error::<T>::NotFound`.
#[test]
fn add_nft_not_found() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(33);
		let unknown_nft = (0, InstanceId(1));

		assert_noop!(
			NftMarketplace::add(seller, unknown_nft.0, unknown_nft.1, CurrencyId::Usd, 3),
			DispatchError::from(nft_marketplace::Error::<Test>::NotFound)
		);
	});
}

#[test]
fn add_nft_no_permission() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(SELLER);
		let (class_id, instance_id) = (0, InstanceId(1));

		// Mint the nft in the uniques pallet
		assert_ok!(Uniques::create(seller.clone(), class_id, SELLER));
		assert_ok!(Uniques::mint(seller.clone(), class_id, instance_id, SELLER));

		// Verify that the seller cannot put the nft for sale because the Nft marketplace
		// was not set as its Admin & Freezer.
		assert_noop!(
			NftMarketplace::add(seller, class_id, instance_id, CurrencyId::Usd, 3),
			DispatchError::from(nft_marketplace::Error::<Test>::NoPermission)
		);
	});
}

#[test]
fn add_nft_works() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(SELLER);
		let (class_id, instance_id) = prepared_nft(&seller);

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

// Verify that if the owner of the nft changes the Admin & Freezer of their nft after adding it,
// calling `NftMarketplace::remove` will still succeed.
#[test]
fn remove_nft_no_permission() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(SELLER);
		let (class_id, instance_id) = prepared_nft(&seller);

		// Set it for sale in the NftMarketplace
		assert_ok!(NftMarketplace::add(
			seller.clone(),
			class_id,
			instance_id,
			CurrencyId::Usd,
			10_000
		));

		// Now we remove NftMarketplace as the admin and freezer of the nft's class
		assert_ok!(Uniques::set_team(
			seller.clone(),
			class_id,
			SELLER,
			SELLER,
			SELLER
		));

		// Verify that try and remove it again fails with `NoPermission`. This happens because
		// we attempt to thaw the nft, frozen when added, and we no longer have the permissions
		// to do so.
		assert_ok!(NftMarketplace::remove(
			seller.clone(),
			class_id,
			instance_id
		));

		// Verify that indeed the nft is no longer for sale
		assert_noop!(
			NftMarketplace::remove(seller, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::NotForSale),
		);
	});
}

#[test]
fn remove_nft_works() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(SELLER);
		let (class_id, instance_id) = prepared_nft(&seller);

		// Set it for sale in the NftMarketplace
		assert_ok!(NftMarketplace::add(
			seller.clone(),
			class_id,
			instance_id,
			CurrencyId::Usd,
			10_000
		));

		assert_ok!(NftMarketplace::remove(
			seller.clone(),
			class_id,
			instance_id
		),);

		// Verify that try and remove it again fails with `NotForSale`
		assert_noop!(
			NftMarketplace::remove(seller, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::NotForSale)
		);
	});
}

#[test]
fn buy_nft_not_found() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(33);
		let unknown_nft = (0, InstanceId(1));

		assert_noop!(
			NftMarketplace::buy(seller, unknown_nft.0, unknown_nft.1),
			DispatchError::from(nft_marketplace::Error::<Test>::NotFound)
		);
	});
}

#[test]
fn buy_nft_already_owner() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(SELLER);
		let (class_id, instance_id) = prepared_nft(&seller);

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
fn buy_nft_not_for_sale() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(SELLER);
		let (class_id, instance_id) = prepared_nft(&seller);

		// Verify that the buyer cannot buy the nft because it's not for sale
		let buyer: Origin = Origin::signed(BUYER);
		assert_noop!(
			NftMarketplace::buy(buyer, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::NotForSale)
		);
	});
}

#[test]
fn buy_nft_insufficient_balance() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(SELLER);
		let (class_id, instance_id) = prepared_nft(&seller);

		// Set it for sale in the NftMarketplace
		assert_ok!(NftMarketplace::add(
			seller.clone(),
			class_id,
			instance_id,
			CurrencyId::Usd,
			OrmlTokens::balance(CurrencyId::Usd, &1) + 1
		));

		// Verify that the buyer cannot buy said nft because its asking price
		// exceeds the seller's balance.
		let buyer: Origin = Origin::signed(BUYER);
		assert_noop!(
			NftMarketplace::buy(buyer, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::InsufficientBalance)
		);
	});
}

#[test]
fn buy_nft_works() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(SELLER);
		let seller_initial_balance = OrmlTokens::balance(CurrencyId::Usd, &1);
		let (class_id, instance_id) = prepared_nft(&seller);

		// Set it for sale in the NftMarketplace
		let nft_price = 10_000;
		assert_ok!(NftMarketplace::add(
			seller.clone(),
			class_id,
			instance_id,
			CurrencyId::Usd,
			nft_price
		));

		// Verify that the buyer can buy the nft
		let buyer: Origin = Origin::signed(BUYER);
		let buyer_initial_balance = OrmlTokens::balance(CurrencyId::Usd, &BUYER);
		assert_ok!(NftMarketplace::buy(buyer.clone(), class_id, instance_id));

		// Verify that if the seller can't buy it back because it's no longer for sale
		assert_noop!(
			NftMarketplace::buy(seller, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::NotForSale)
		);

		// Verify that if the seller can't buy it back because it's no longer for sale
		assert_eq!(Uniques::owner(class_id, instance_id), Some(BUYER));

		// Verify that the price of the nft was transferred to the seller's account
		assert_eq!(
			OrmlTokens::balance(CurrencyId::Usd, &SELLER),
			seller_initial_balance + nft_price
		);

		// Verify that the price of the nft was withdrawn from the buyer's account
		assert_eq!(
			OrmlTokens::balance(CurrencyId::Usd, &BUYER),
			buyer_initial_balance - nft_price
		);
	});
}

/// Create an NFT in the Uniques pallet and set the nft marketplace as its owner & admin.
/// Return the identifier of the nft, `(class_id, instance_id)`.
fn prepared_nft(owner: &Origin) -> (u64, InstanceId) {
	let (class_id, instance_id) = (0, InstanceId(1));

	// Mint the nft in the uniques pallet
	assert_ok!(Uniques::create(owner.clone(), class_id, SELLER));
	assert_ok!(Uniques::mint(owner.clone(), class_id, instance_id, SELLER));

	assert_ok!(Uniques::set_team(
		owner.clone(),
		class_id,
		SELLER,
		NftMarketplace::account(),
		NftMarketplace::account()
	));

	(class_id, instance_id)
}
