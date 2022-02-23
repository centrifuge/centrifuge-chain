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
		let seller: Origin = Origin::signed(SELLER);
		let unknown_nft = (0, InstanceId(1));

		assert_noop!(
			NftMarketplace::add(seller, unknown_nft.0, unknown_nft.1, CurrencyId::Usd, 3),
			DispatchError::from(nft_marketplace::Error::<Test>::NotFound)
		);
	});
}

/// Verify that a bad actor cannot put another user's NFTs for sale
#[test]
fn add_nft_not_owner() {
	new_test_ext().execute_with(|| {
		let owner: Origin = Origin::signed(SELLER);
		let (class_id, instance_id) = prepared_nft(&owner);

		let bad_actor = Origin::signed(BUYER);
		assert_noop!(
			NftMarketplace::add(bad_actor, class_id, instance_id, CurrencyId::Usd, 3),
			DispatchError::from(nft_marketplace::Error::<Test>::NotOwner)
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

		// Verify that if the seller tries to put it for sale again, that it fails with `NotOwner`
		// given that the NFT is not owned by the nft-marketplace pallet.
		assert_noop!(
			NftMarketplace::add(seller, class_id, instance_id, CurrencyId::Usd, 10_000),
			DispatchError::from(nft_marketplace::Error::<Test>::NotOwner)
		);

		// Verify that if the nft-marketplace pallet would go on trying to add it again,
		// it would fail with `AlreadyForSale`.
		assert_noop!(
			NftMarketplace::add(
				NftMarketplace::origin(),
				class_id,
				instance_id,
				CurrencyId::Usd,
				10_000
			),
			DispatchError::from(nft_marketplace::Error::<Test>::AlreadyForSale)
		);
	});
}

// Verify that a bad actor cannot remove someone else's NFT from sale.
#[test]
fn remove_nft_bad_actor() {
	new_test_ext().execute_with(|| {
		let seller: Origin = Origin::signed(SELLER);
		let (class_id, instance_id) = prepared_nft(&seller);

		// Set it for sale in the NftMarketplace
		assert_ok!(NftMarketplace::add(
			seller,
			class_id,
			instance_id,
			CurrencyId::Usd,
			10_000
		));

		let bad_actor = Origin::signed(BUYER);
		assert_noop!(
			NftMarketplace::remove(bad_actor.clone(), class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::NotOwner)
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
		));

		// Verify that try and remove it again fails with `NotForSale`
		assert_noop!(
			NftMarketplace::remove(seller, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::NotForSale)
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

		// Verify that the seller cannot buy the item they are selling
		assert_noop!(
			NftMarketplace::buy(seller, class_id, instance_id),
			DispatchError::from(nft_marketplace::Error::<Test>::IsSeller)
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
			OrmlTokens::balance(CurrencyId::Usd, &1) + 1 // < Just too expensive
		));

		// Verify that the buyer cannot buy the nft because its asking price
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

/// Mint an NFT class and instance and return its `(class_id, instance_id)`
fn prepared_nft(owner: &Origin) -> (u64, InstanceId) {
	let (class_id, instance_id) = (0, InstanceId(1));

	// Mint the nft in the uniques pallet
	assert_ok!(Uniques::create(owner.clone(), class_id, SELLER));
	assert_ok!(Uniques::mint(owner.clone(), class_id, instance_id, SELLER));

	(class_id, instance_id)
}
