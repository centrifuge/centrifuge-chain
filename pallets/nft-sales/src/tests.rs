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

use cfg_primitives::ItemId;
use cfg_types::tokens::CurrencyId;
use frame_support::{assert_noop, assert_ok, dispatch::DispatchError, traits::fungibles::Inspect};

use crate::{mock::*, NftsBySeller, Price};

pub const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

/// Verify that calling `NftSales::add` specifiying an nft that is not present
/// in the underlying `pallet_uniques` fails with
/// `nft_sales::Error::<T>::NotFound`.
#[test]
fn add_nft_not_found() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let unknown_nft = (0, ItemId(1));

		assert_noop!(
			NftSales::add(
				seller,
				unknown_nft.0,
				unknown_nft.1,
				Price {
					currency: AUSD_CURRENCY_ID,
					amount: 3
				}
			),
			DispatchError::from(nft_sales::Error::<Runtime>::NotFound)
		);
	});
}

/// Verify that a bad actor cannot put another user's NFTs for sale
#[test]
fn add_nft_not_owner() {
	new_test_ext().execute_with(|| {
		let owner: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let (collection_id, item_id) = prepared_nft(&owner);

		let bad_actor = RuntimeOrigin::signed(BAD_ACTOR);
		assert_noop!(
			NftSales::add(
				bad_actor,
				collection_id,
				item_id,
				Price {
					currency: AUSD_CURRENCY_ID,
					amount: 3
				}
			),
			DispatchError::from(nft_sales::Error::<Runtime>::NotOwner)
		);

		// Verify that the NFT is not listed under the BAD_ACTOR
		assert!(!NftsBySeller::<Runtime>::contains_key((
			BAD_ACTOR,
			collection_id,
			item_id
		)));
	});
}

#[test]
fn add_nft_works() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let (collection_id, item_id) = prepared_nft(&seller);
		let price = Price {
			currency: AUSD_CURRENCY_ID,
			amount: 10_000,
		};

		// Set it for sale in the NftSales
		assert_ok!(NftSales::add(
			seller.clone(),
			collection_id,
			item_id,
			price.clone(),
		));

		// Verify that if the seller tries to put it for sale again, that it fails with
		// `NotOwner` given that the NFT is not owned by the nft-sales pallet.
		assert_noop!(
			NftSales::add(seller, collection_id, item_id, price.clone()),
			DispatchError::from(nft_sales::Error::<Runtime>::NotOwner)
		);

		// Verify that if the nft-sales pallet would go on trying to add it again,
		// it would fail with `AlreadyForSale`.
		assert_noop!(
			NftSales::add(NftSales::origin(), collection_id, item_id, price),
			DispatchError::from(nft_sales::Error::<Runtime>::AlreadyForSale)
		);

		// Verify that the nft is now listed in the storage
		assert!(NftsBySeller::<Runtime>::contains_key((
			SELLER,
			collection_id,
			item_id
		)));
	});
}

// Verify that a bad actor cannot remove someone else's NFT from sale.
#[test]
fn remove_nft_bad_actor() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let (collection_id, item_id) = prepared_nft(&seller);
		let price = Price {
			currency: AUSD_CURRENCY_ID,
			amount: 10_000,
		};

		// Set it for sale in the NftSales
		assert_ok!(NftSales::add(seller, collection_id, item_id, price));

		// Verify that the nft is now listed in the storage
		assert!(NftsBySeller::<Runtime>::contains_key((
			SELLER,
			collection_id,
			item_id
		)));

		// Have a bad actor trying to remove it
		let bad_actor = RuntimeOrigin::signed(BUYER);
		assert_noop!(
			NftSales::remove(bad_actor, collection_id, item_id),
			DispatchError::from(nft_sales::Error::<Runtime>::NotOwner)
		);

		// Verify that the nft is still listed
		assert!(NftsBySeller::<Runtime>::contains_key((
			SELLER,
			collection_id,
			item_id
		)));
	});
}

#[test]
fn remove_nft_works() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let (collection_id, item_id) = prepared_nft(&seller);
		let price = Price {
			currency: AUSD_CURRENCY_ID,
			amount: 10_000,
		};

		// Add it for sale
		assert_ok!(NftSales::add(seller.clone(), collection_id, item_id, price));

		// Verify that it's now stored
		assert!(NftsBySeller::<Runtime>::contains_key((
			SELLER,
			collection_id,
			item_id
		)));

		assert_ok!(NftSales::remove(seller.clone(), collection_id, item_id));

		// Verify that the nft is no longer listed in the storage
		assert!(!NftsBySeller::<Runtime>::contains_key((
			SELLER,
			collection_id,
			item_id
		)));

		// Verify that try and remove it again fails with `NotForSale`
		assert_noop!(
			NftSales::remove(seller, collection_id, item_id),
			DispatchError::from(nft_sales::Error::<Runtime>::NotForSale)
		);
	});
}

// Verify that a seller of an NFT can choose to buy it.
#[test]
fn buy_nft_seller() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let (collection_id, item_id) = prepared_nft(&seller);
		let price = Price {
			currency: AUSD_CURRENCY_ID,
			amount: 10_000,
		};
		// Set it for sale in the NftSales
		assert_ok!(NftSales::add(
			seller.clone(),
			collection_id,
			item_id,
			price.clone(),
		));

		assert_ok!(NftSales::buy(seller, collection_id, item_id, price));
	});
}

#[test]
fn buy_nft_not_for_sale() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let (collection_id, item_id) = prepared_nft(&seller);
		let offer = Price {
			currency: AUSD_CURRENCY_ID,
			amount: 10_000,
		};

		// Verify that the buyer cannot buy the nft because it's not for sale
		let buyer: RuntimeOrigin = RuntimeOrigin::signed(BUYER);
		assert_noop!(
			NftSales::buy(buyer, collection_id, item_id, offer),
			DispatchError::from(nft_sales::Error::<Runtime>::NotForSale)
		);
	});
}

#[test]
fn buy_nft_insufficient_balance() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let (collection_id, item_id) = prepared_nft(&seller);
		let price = Price {
			currency: AUSD_CURRENCY_ID,
			amount: OrmlTokens::balance(AUSD_CURRENCY_ID, &1) + 1,
		};

		// Set it for sale in the NftSales
		assert_ok!(NftSales::add(
			seller.clone(),
			collection_id,
			item_id,
			price.clone(), // < Just too expensive
		));

		// Verify that the buyer cannot buy the nft because its asking price
		// exceeds the seller's balance.
		let buyer: RuntimeOrigin = RuntimeOrigin::signed(BUYER);
		assert_noop!(
			NftSales::buy(buyer, collection_id, item_id, price),
			DispatchError::from(orml_tokens::Error::<Runtime>::BalanceTooLow)
		);
	});
}

#[test]
fn buy_nft_works() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let seller_initial_balance = OrmlTokens::balance(AUSD_CURRENCY_ID, &1);
		let (collection_id, item_id) = prepared_nft(&seller);
		let price = Price {
			currency: AUSD_CURRENCY_ID,
			amount: 10_000,
		};

		// Add it for sale
		assert_ok!(NftSales::add(
			seller.clone(),
			collection_id,
			item_id,
			price.clone(),
		));

		// Verify that the nft is now listed in the storage
		assert!(NftsBySeller::<Runtime>::contains_key((
			SELLER,
			collection_id,
			item_id
		)));

		// Verify that the buyer can buy the nft
		let buyer: RuntimeOrigin = RuntimeOrigin::signed(BUYER);
		let buyer_initial_balance = OrmlTokens::balance(AUSD_CURRENCY_ID, &BUYER);
		assert_ok!(NftSales::buy(
			buyer.clone(),
			collection_id,
			item_id,
			price.clone()
		));

		// Verify that if the seller can't buy it back because it's no longer for sale
		assert_noop!(
			NftSales::buy(seller, collection_id, item_id, price.clone()),
			DispatchError::from(nft_sales::Error::<Runtime>::NotForSale)
		);

		// Verify that if the seller can't buy it back because it's no longer for sale
		assert_eq!(Uniques::owner(collection_id, item_id), Some(BUYER));

		// Verify that the price of the nft was transferred to the seller's account
		assert_eq!(
			OrmlTokens::balance(price.currency, &SELLER),
			seller_initial_balance + price.amount
		);

		// Verify that the price of the nft was withdrawn from the buyer's account
		assert_eq!(
			OrmlTokens::balance(price.currency, &BUYER),
			buyer_initial_balance - price.amount
		);

		// Verify that the nft is no longer listed
		assert!(!NftsBySeller::<Runtime>::contains_key((
			SELLER,
			collection_id,
			item_id
		)));
	});
}

// Verify that the max offer amount of the buyer is respected. If it's lower
// than the asking price, it should fail with `InvalidOffer`
#[test]
fn buy_nft_respects_max_offer_amount() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let (collection_id, item_id) = prepared_nft(&seller);
		let price = Price {
			currency: AUSD_CURRENCY_ID,
			amount: 10_000,
		};

		// Add it for sale
		assert_ok!(NftSales::add(
			seller.clone(),
			collection_id,
			item_id,
			price.clone(),
		));

		let buyer: RuntimeOrigin = RuntimeOrigin::signed(BUYER);
		let offer = Price {
			currency: price.currency,
			amount: price.amount - 1,
		};
		assert_noop!(
			NftSales::buy(buyer.clone(), collection_id, item_id, offer),
			DispatchError::from(nft_sales::Error::<Runtime>::InvalidOffer)
		);
	});
}

// Verify that the max offer amount of the buyer is respected. If it's lower
// than the asking price, it should fail with `InvalidOffer`
#[test]
fn buy_nft_respects_max_offer_currency() {
	new_test_ext().execute_with(|| {
		let seller: RuntimeOrigin = RuntimeOrigin::signed(SELLER);
		let (collection_id, item_id) = prepared_nft(&seller);
		let price = Price {
			currency: AUSD_CURRENCY_ID,
			amount: 10_000,
		};

		// Add it for sale
		assert_ok!(NftSales::add(
			seller.clone(),
			collection_id,
			item_id,
			price.clone(),
		));

		let buyer: RuntimeOrigin = RuntimeOrigin::signed(BUYER);
		let offer = Price {
			currency: CurrencyId::Native, // <- mismatching currency
			amount: price.amount,
		};

		assert_noop!(
			NftSales::buy(buyer.clone(), collection_id, item_id, offer),
			DispatchError::from(nft_sales::Error::<Runtime>::InvalidOffer)
		);
	});
}

/// Mint an NFT class and instance and return its `(collection_id, item_id)`
fn prepared_nft(owner: &RuntimeOrigin) -> (u64, ItemId) {
	let (collection_id, item_id) = (0, ItemId(1));

	assert_ok!(Uniques::create(owner.clone(), collection_id, SELLER));
	assert_ok!(Uniques::mint(owner.clone(), collection_id, item_id, SELLER));

	(collection_id, item_id)
}
