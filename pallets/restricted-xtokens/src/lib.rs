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
#![cfg_attr(not(feature = "std"), no_std)]

//! A crate that allows for checking of preconditions before sending tokens via xcm.
//! Mimics orml-xtokens api.

pub use pallet::*;
pub use weights::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

use xcm::{latest::Weight, prelude::*};

pub enum Effects<AccountId, CurrencyId, Balance> {
	Transfer {
		send: AccountId,
		recv: MultiLocation,
		id: CurrencyId,
		amount: Balance,
		dest_weight_limit: Weight,
	},
	TransferMultiAsset {
		send: AccountId,
		recv: MultiLocation,
		id: MultiAsset,
		dest_weight_limit: Weight,
	},
	TransferWithFee {
		send: AccountId,
		recv: MultiLocation,
		id: CurrencyId,
		amount: Balance,
		fee: Balance,
		dest_weight_limit: Weight,
	},
	TransferMultiAssetWithFee {
		send: AccountId,
		recv: MultiLocation,
		id: MultiAsset,
		fee_asset: MultiAsset,
		dest_weight_limit: Weight,
	},
	TransferMultiCurrencies {
		send: AccountId,
		recv: MultiLocation,
		transfers: Vec<(CurrencyId, Balance)>,
		fee_item: u32,
		dest_weight_limit: Weight,
	},
}

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::PreConditions;
	use frame_system::pallet_prelude::{ensure_signed, OriginFor};
	use orml_xtokens::Pallet as XTokens;
	use sp_runtime::DispatchResult;

	use super::*;

	type EffectsOf<T> = super::Effects<
		<T as frame_system::Config>::AccountId,
		<T as orml_xtokens::Config>::CurrencyId,
		<T as orml_xtokens::Config>::Balance,
	>;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + orml_xtokens::Config {
		/// Checks the pre conditions for every transfer via the user api (i.e. extrinsics)
		type PreExtrTransfer: PreConditions<EffectsOf<Self>, Result = DispatchResult>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// The version of the `Versioned` value used is not able to be
		/// interpreted.
		BadVersion,
	}

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
		#[pallet::weight(0)]
		pub fn transfer(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: Weight,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			let recv: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| Error::<T>::BadVersion)?;

			T::PreExtrTransfer::check(EffectsOf::<T>::Transfer {
				send: who,
				recv,
				id: currency_id.clone(),
				amount,
				dest_weight_limit,
			})?;

			XTokens::<T>::transfer(origin, currency_id, amount, dest, dest_weight_limit)
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
		#[pallet::weight(0)]
		pub fn transfer_multiasset(
			origin: OriginFor<T>,
			asset: Box<VersionedMultiAsset>,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: Weight,
		) -> DispatchResult {
			let send = ensure_signed(origin.clone())?;
			let id: MultiAsset = (*asset.clone())
				.try_into()
				.map_err(|()| Error::<T>::BadVersion)?;
			let recv: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| Error::<T>::BadVersion)?;

			T::PreExtrTransfer::check(EffectsOf::<T>::TransferMultiAsset {
				send,
				recv,
				id,
				dest_weight_limit,
			})?;

			XTokens::<T>::transfer_multiasset(origin, asset, dest, dest_weight_limit)
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
		#[pallet::weight(0)]
		pub fn transfer_with_fee(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
			fee: T::Balance,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: Weight,
		) -> DispatchResult {
			let send = ensure_signed(origin.clone())?;
			let recv: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| Error::<T>::BadVersion)?;

			T::PreExtrTransfer::check(EffectsOf::<T>::TransferWithFee {
				send,
				recv,
				id: currency_id.clone(),
				amount,
				fee,
				dest_weight_limit,
			})?;

			XTokens::<T>::transfer_with_fee(
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
		#[pallet::weight(0)]
		pub fn transfer_multiasset_with_fee(
			origin: OriginFor<T>,
			asset: Box<VersionedMultiAsset>,
			fee: Box<VersionedMultiAsset>,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: Weight,
		) -> DispatchResult {
			let send = ensure_signed(origin.clone())?;
			let id: MultiAsset = (*asset.clone())
				.try_into()
				.map_err(|()| Error::<T>::BadVersion)?;
			let fee_asset: MultiAsset = (*fee.clone())
				.try_into()
				.map_err(|()| Error::<T>::BadVersion)?;
			let recv: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| Error::<T>::BadVersion)?;

			T::PreExtrTransfer::check(EffectsOf::<T>::TransferMultiAssetWithFee {
				send,
				recv,
				id,
				fee_asset,
				dest_weight_limit,
			})?;

			XTokens::<T>::transfer_multiasset_with_fee(origin, asset, fee, dest, dest_weight_limit)
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
		#[pallet::weight(0)]
		pub fn transfer_multicurrencies(
			origin: OriginFor<T>,
			currencies: Vec<(T::CurrencyId, T::Balance)>,
			fee_item: u32,
			dest: Box<VersionedMultiLocation>,
			dest_weight_limit: Weight,
		) -> DispatchResult {
			let send = ensure_signed(origin.clone())?;
			let recv: MultiLocation = (*dest.clone())
				.try_into()
				.map_err(|()| Error::<T>::BadVersion)?;

			T::PreExtrTransfer::check(EffectsOf::<T>::TransferMultiCurrencies {
				send,
				recv,
				transfers: currencies.clone(),
				fee_item,
				dest_weight_limit,
			})?;

			XTokens::<T>::transfer_multicurrencies(
				origin,
				currencies,
				fee_item,
				dest,
				dest_weight_limit,
			)
		}
	}
}
