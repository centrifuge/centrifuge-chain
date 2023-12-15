// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

use cfg_primitives::{AccountId, Balance};
use cfg_traits::{PreConditions, TransferAllowance};
use cfg_types::{domain_address::DomainAddress, locations::Location, tokens::CurrencyId};
use codec::Encode;
use pallet_restricted_tokens::TransferDetails;
use pallet_restricted_xtokens::TransferEffects;
use sp_core::Hasher;
use sp_runtime::{
	traits::{BlakeTwo256, Convert},
	DispatchError, DispatchResult, TokenError,
};
use xcm::v3::{MultiAsset, MultiLocation};

pub struct PreXcmTransfer<T, C>(sp_std::marker::PhantomData<(T, C)>);

impl<
		T: TransferAllowance<AccountId, CurrencyId = CurrencyId, Location = Location>,
		C: Convert<MultiAsset, Option<CurrencyId>>,
	> PreConditions<TransferEffects<AccountId, CurrencyId, Balance>> for PreXcmTransfer<T, C>
{
	type Result = DispatchResult;

	fn check(t: TransferEffects<AccountId, CurrencyId, Balance>) -> Self::Result {
		let currency_based_check = |sender, destination: MultiLocation, currency| {
			T::allowance(
				sender,
				Location::XCM(BlakeTwo256::hash(&destination.encode())),
				currency,
			)
		};

		let asset_based_check = |sender, destination, asset| {
			let currency =
				C::convert(asset).ok_or(DispatchError::Token(TokenError::UnknownAsset))?;

			currency_based_check(sender, destination, currency)
		};

		match t {
			TransferEffects::Transfer {
				sender,
				destination,
				currency_id,
				..
			} => currency_based_check(sender, destination, currency_id),
			TransferEffects::TransferMultiAsset {
				sender,
				destination,
				asset,
			} => asset_based_check(sender, destination, asset),
			TransferEffects::TransferWithFee {
				sender,
				destination,
				currency_id,
				..
			} => currency_based_check(sender, destination, currency_id),
			TransferEffects::TransferMultiAssetWithFee {
				sender,
				destination,
				asset,
				fee_asset,
			} => {
				asset_based_check(sender.clone(), destination, asset)?;

				// NOTE: We do check the fee asset and assume that the destination
				//       is the same as for the actual assets. This is a pure subjective
				//       security assumption to not allow randomly burning fees of
				//       protected assets.
				asset_based_check(sender, destination, fee_asset)
			}
			TransferEffects::TransferMultiCurrencies {
				sender,
				destination,
				currencies,
				fee,
			} => {
				for (currency, ..) in currencies {
					currency_based_check(sender.clone(), destination, currency)?;
				}

				// NOTE: We do check the fee asset and assume that the destination
				//       is the same as for the actual assets. This is a pure subjective
				//       security assumption to not allow randomly burning fees of
				//       protected assets.
				currency_based_check(sender, destination, fee.0)
			}
			TransferEffects::TransferMultiAssets {
				sender,
				destination,
				assets,
				fee_asset,
			} => {
				// NOTE: We do not check the fee, as we assume, that this is not a transfer
				//       but rather a burn of tokens. Furthermore, we do not know the
				//       destination where those fees will go.
				for asset in assets.into_inner() {
					asset_based_check(sender.clone(), destination, asset)?;
				}

				// NOTE: We do check the fee asset and assume that the destination
				//       is the same as for the actual assets. This is a pure subjective
				//       security assumption to not allow randomly burning fees of
				//       protected assets.
				asset_based_check(sender, destination, fee_asset)
			}
		}
	}
}

pub struct PreNativeTransfer<T>(sp_std::marker::PhantomData<T>);

impl<T: TransferAllowance<AccountId, CurrencyId = CurrencyId, Location = Location>>
	PreConditions<TransferDetails<AccountId, CurrencyId, Balance>> for PreNativeTransfer<T>
{
	type Result = bool;

	fn check(t: TransferDetails<AccountId, CurrencyId, Balance>) -> Self::Result {
		T::allowance(t.send, Location::Local(t.recv), t.id).is_ok()
	}
}
pub struct PreLpTransfer<T>(sp_std::marker::PhantomData<T>);

impl<T: TransferAllowance<AccountId, CurrencyId = CurrencyId, Location = Location>>
	PreConditions<(AccountId, DomainAddress, CurrencyId)> for PreLpTransfer<T>
{
	type Result = DispatchResult;

	fn check(t: (AccountId, DomainAddress, CurrencyId)) -> Self::Result {
		let (sender, receiver, currency) = t;
		T::allowance(sender, Location::Address(receiver), currency)
	}
}
