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

//! # Xtokens Module
//!
//! ## Overview
//!
//! The xtokens module provides cross-chain token transfer functionality, by
//! cross-consensus messages(XCM).
//!
//! The xtokens module provides functions for
//! - Token transfer from parachains to relay chain.
//! - Token transfer between parachains, including relay chain tokens like DOT,
//!   KSM, and parachain tokens like ACA, aUSD.
//!
//! ## Interface
//!
//! ### Dispatchable functions
//!
//! - `transfer`: Transfer local assets with given `CurrencyId` and `Amount`.
//! - `transfer_multiasset`: Transfer `MultiAsset` assets.
//! - `transfer_with_fee`: Transfer native currencies specifying the fee and
//!   amount as separate.
//! - `transfer_multiasset_with_fee`: Transfer `MultiAsset` specifying the fee
//!   and amount as separate.
//! - `transfer_multicurrencies`: Transfer several currencies specifying the
//!   item to be used as fee.
//! - `transfer_multiassets`: Transfer several `MultiAsset` specifying the item
//!   to be used as fee.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::from_over_into)]
#![allow(clippy::unused_unit)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::boxed_local)]
#![allow(clippy::too_many_arguments)]

use cfg_traits::PreConditions;
use frame_support::pallet_prelude::*;
use frame_system::{ensure_signed, pallet_prelude::*};
use orml_traits::XtokensWeightInfo;
pub use pallet::*;
use sp_std::{boxed::Box, vec::Vec};
use xcm::{v3::prelude::*, VersionedMultiAsset, VersionedMultiAssets, VersionedMultiLocation};

mod mock;
mod tests;

pub enum TransferEffects<AccountId, CurrencyId, Balance> {
	Transfer {
		sender: AccountId,
		destination: MultiLocation,
		currency_id: CurrencyId,
		amount: Balance,
	},
	TransferMultiAsset {
		sender: AccountId,
		destination: MultiLocation,
		asset: MultiAsset,
	},
	TransferWithFee {
		sender: AccountId,
		destination: MultiLocation,
		currency_id: CurrencyId,
		amount: Balance,
		fee: Balance,
	},
	TransferMultiAssetWithFee {
		sender: AccountId,
		destination: MultiLocation,
		asset: MultiAsset,
		fee_asset: MultiAsset,
	},
	TransferMultiCurrencies {
		sender: AccountId,
		destination: MultiLocation,
		currencies: Vec<(CurrencyId, Balance)>,
		fee: (CurrencyId, Balance),
	},
	TransferMultiAssets {
		sender: AccountId,
		destination: MultiLocation,
		assets: MultiAssets,
		fee_asset: MultiAsset,
	},
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + orml_xtokens::Config {
		type PreTransfer: PreConditions<
			TransferEffects<Self::AccountId, Self::CurrencyId, Self::Balance>,
			Result = bool,
		>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Transfer has been restricted by the runtime.
		/// In most cases this means there exist a restriction on the sender and
		/// the receiver of the transfer is not allowlisted as a receiver
		RestrictionTriggered,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Transfer native currencies.
		///
		/// `dest_weight_limit` is the weight for XCM execution on the dest
		/// chain, and it would be charged from the transferred assets. If set
		/// below requirements, the execution may fail and assets wouldn't be
		/// received.
		///
		/// It's a no-op if any error on local XCM execution or message sending.
		/// Note sending assets out per se doesn't guarantee they would be
		/// received. Receiving depends on if the XCM message could be delivered
		/// by the network, and if the receiving chain would handle
		/// messages correctly.
		#[pallet::call_index(0)]
		#[pallet::weight(orml_xtokens::XtokensWeight::< T >::weight_of_transfer(currency_id.clone(), * amount, dest))]
		pub fn transfer(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: WeightLimit,
		) -> DispatchResult {
			let destination: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;
			let sender = ensure_signed(origin.clone())?;

			ensure!(
				T::PreTransfer::check(TransferEffects::Transfer {
					sender,
					destination,
					currency_id: currency_id.clone(),
					amount
				}),
				Error::<T>::RestrictionTriggered
			);

			orml_xtokens::Pallet::<T>::transfer(
				origin,
				currency_id,
				amount,
				dest,
				dest_weight_limit,
			)
		}

		/// Transfer `MultiAsset`.
		///
		/// `dest_weight_limit` is the weight for XCM execution on the dest
		/// chain, and it would be charged from the transferred assets. If set
		/// below requirements, the execution may fail and assets wouldn't be
		/// received.
		///
		/// It's a no-op if any error on local XCM execution or message sending.
		/// Note sending assets out per se doesn't guarantee they would be
		/// received. Receiving depends on if the XCM message could be delivered
		/// by the network, and if the receiving chain would handle
		/// messages correctly.
		#[pallet::call_index(1)]
		#[pallet::weight(orml_xtokens::XtokensWeight::< T >::weight_of_transfer_multiasset(asset, dest))]
		pub fn transfer_multiasset(
			origin: OriginFor<T>,
			asset: Box<VersionedMultiAsset>,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: WeightLimit,
		) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			let multi_asset: MultiAsset = (*asset.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;
			let destination: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;

			ensure!(
				T::PreTransfer::check(TransferEffects::TransferMultiAsset {
					sender,
					destination,
					asset: multi_asset,
				}),
				Error::<T>::RestrictionTriggered
			);

			orml_xtokens::Pallet::<T>::transfer_multiasset(origin, asset, dest, dest_weight_limit)
		}

		/// Transfer native currencies specifying the fee and amount as
		/// separate.
		///
		/// `dest_weight_limit` is the weight for XCM execution on the dest
		/// chain, and it would be charged from the transferred assets. If set
		/// below requirements, the execution may fail and assets wouldn't be
		/// received.
		///
		/// `fee` is the amount to be spent to pay for execution in destination
		/// chain. Both fee and amount will be subtracted form the callers
		/// balance.
		///
		/// If `fee` is not high enough to cover for the execution costs in the
		/// destination chain, then the assets will be trapped in the
		/// destination chain
		///
		/// It's a no-op if any error on local XCM execution or message sending.
		/// Note sending assets out per se doesn't guarantee they would be
		/// received. Receiving depends on if the XCM message could be delivered
		/// by the network, and if the receiving chain would handle
		/// messages correctly.
		#[pallet::call_index(2)]
		#[pallet::weight(orml_xtokens::XtokensWeight::< T >::weight_of_transfer(currency_id.clone(), * amount, dest))]
		pub fn transfer_with_fee(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
			fee: T::Balance,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: WeightLimit,
		) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			let destination: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;

			ensure!(
				T::PreTransfer::check(TransferEffects::TransferWithFee {
					sender,
					destination,
					currency_id: currency_id.clone(),
					amount,
					fee
				}),
				Error::<T>::RestrictionTriggered
			);

			orml_xtokens::Pallet::<T>::transfer_with_fee(
				origin,
				currency_id,
				amount,
				fee,
				dest,
				dest_weight_limit,
			)
		}

		/// Transfer `MultiAsset` specifying the fee and amount as separate.
		///
		/// `dest_weight_limit` is the weight for XCM execution on the dest
		/// chain, and it would be charged from the transferred assets. If set
		/// below requirements, the execution may fail and assets wouldn't be
		/// received.
		///
		/// `fee` is the multiasset to be spent to pay for execution in
		/// destination chain. Both fee and amount will be subtracted form the
		/// callers balance For now we only accept fee and asset having the same
		/// `MultiLocation` id.
		///
		/// If `fee` is not high enough to cover for the execution costs in the
		/// destination chain, then the assets will be trapped in the
		/// destination chain
		///
		/// It's a no-op if any error on local XCM execution or message sending.
		/// Note sending assets out per se doesn't guarantee they would be
		/// received. Receiving depends on if the XCM message could be delivered
		/// by the network, and if the receiving chain would handle
		/// messages correctly.
		#[pallet::call_index(3)]
		#[pallet::weight(orml_xtokens::XtokensWeight::< T >::weight_of_transfer_multiasset(asset, dest))]
		pub fn transfer_multiasset_with_fee(
			origin: OriginFor<T>,
			asset: Box<VersionedMultiAsset>,
			fee: Box<VersionedMultiAsset>,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: WeightLimit,
		) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			let multi_asset: MultiAsset = (*asset.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;
			let fee_asset: MultiAsset = (*fee.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;
			let destination: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;

			ensure!(
				T::PreTransfer::check(TransferEffects::TransferMultiAssetWithFee {
					sender,
					destination,
					asset: multi_asset,
					fee_asset
				}),
				Error::<T>::RestrictionTriggered
			);

			orml_xtokens::Pallet::<T>::transfer_multiasset_with_fee(
				origin,
				asset,
				fee,
				dest,
				dest_weight_limit,
			)
		}

		/// Transfer several currencies specifying the item to be used as fee
		///
		/// `dest_weight_limit` is the weight for XCM execution on the dest
		/// chain, and it would be charged from the transferred assets. If set
		/// below requirements, the execution may fail and assets wouldn't be
		/// received.
		///
		/// `fee_item` is index of the currencies tuple that we want to use for
		/// payment
		///
		/// It's a no-op if any error on local XCM execution or message sending.
		/// Note sending assets out per se doesn't guarantee they would be
		/// received. Receiving depends on if the XCM message could be delivered
		/// by the network, and if the receiving chain would handle
		/// messages correctly.
		#[pallet::call_index(4)]
		#[pallet::weight(orml_xtokens::XtokensWeight::< T >::weight_of_transfer_multicurrencies(currencies, fee_item, dest))]
		pub fn transfer_multicurrencies(
			origin: OriginFor<T>,
			currencies: Vec<(T::CurrencyId, T::Balance)>,
			fee_item: u32,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: WeightLimit,
		) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			let destination: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;
			let fee = currencies
				.get(fee_item as usize)
				.ok_or(orml_xtokens::Error::<T>::AssetIndexNonExistent)?;

			ensure!(
				T::PreTransfer::check(TransferEffects::TransferMultiCurrencies {
					sender,
					destination,
					currencies: currencies.clone(),
					fee: fee.clone()
				}),
				Error::<T>::RestrictionTriggered
			);

			orml_xtokens::Pallet::<T>::transfer_multicurrencies(
				origin,
				currencies,
				fee_item,
				dest,
				dest_weight_limit,
			)
		}

		/// Transfer several `MultiAsset` specifying the item to be used as fee
		///
		/// `dest_weight_limit` is the weight for XCM execution on the dest
		/// chain, and it would be charged from the transferred assets. If set
		/// below requirements, the execution may fail and assets wouldn't be
		/// received.
		///
		/// `fee_item` is index of the MultiAssets that we want to use for
		/// payment
		///
		/// It's a no-op if any error on local XCM execution or message sending.
		/// Note sending assets out per se doesn't guarantee they would be
		/// received. Receiving depends on if the XCM message could be delivered
		/// by the network, and if the receiving chain would handle
		/// messages correctly.
		#[pallet::call_index(5)]
		#[pallet::weight(orml_xtokens::XtokensWeight::< T >::weight_of_transfer_multiassets(assets, fee_item, dest))]
		pub fn transfer_multiassets(
			origin: OriginFor<T>,
			assets: Box<VersionedMultiAssets>,
			fee_item: u32,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: WeightLimit,
		) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			let multi_assets: MultiAssets = (*assets.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;
			let destination: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| orml_xtokens::Error::<T>::BadVersion)?;
			let fee_asset: &MultiAsset = multi_assets
				.get(fee_item as usize)
				.ok_or(orml_xtokens::Error::<T>::AssetIndexNonExistent)?;

			ensure!(
				T::PreTransfer::check(TransferEffects::TransferMultiAssets {
					sender,
					destination,
					assets: multi_assets.clone(),
					fee_asset: fee_asset.clone()
				}),
				Error::<T>::RestrictionTriggered
			);

			orml_xtokens::Pallet::<T>::transfer_multiassets(
				origin,
				assets,
				fee_item,
				dest,
				dest_weight_limit,
			)
		}
	}
}
