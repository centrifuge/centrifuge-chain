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
use cfg_types::{
	domain_address::DomainAddress,
	locations::Location,
	tokens::{CurrencyId, FilterCurrency},
};
use frame_support::{dispatch::TypeInfo, traits::IsSubType, RuntimeDebugNoBound};
use pallet_restricted_tokens::TransferDetails;
use pallet_restricted_xtokens::TransferEffects;
use parity_scale_codec::{Decode, Encode};
use sp_core::Hasher;
use sp_runtime::{
	traits::{BlakeTwo256, Convert, DispatchInfoOf, SignedExtension, StaticLookup},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	DispatchError, DispatchResult, TokenError,
};
use xcm::v3::{MultiAsset, MultiLocation};

pub struct PreXcmTransfer<T, C>(sp_std::marker::PhantomData<(T, C)>);

impl<
		T: TransferAllowance<AccountId, CurrencyId = FilterCurrency, Location = Location>,
		C: Convert<MultiAsset, Option<CurrencyId>>,
	> PreConditions<TransferEffects<AccountId, CurrencyId, Balance>> for PreXcmTransfer<T, C>
{
	type Result = DispatchResult;

	fn check(t: TransferEffects<AccountId, CurrencyId, Balance>) -> Self::Result {
		let currency_based_check = |sender: AccountId, destination: MultiLocation, currency| {
			T::allowance(
				sender.clone(),
				Location::XCM(BlakeTwo256::hash(&destination.encode())),
				FilterCurrency::Specific(currency),
			)
			.or_else(|_| {
				T::allowance(
					sender,
					Location::XCM(BlakeTwo256::hash(&destination.encode())),
					FilterCurrency::All,
				)
			})
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

impl<T: TransferAllowance<AccountId, CurrencyId = FilterCurrency, Location = Location>>
	PreConditions<TransferDetails<AccountId, CurrencyId, Balance>> for PreNativeTransfer<T>
{
	type Result = bool;

	fn check(t: TransferDetails<AccountId, CurrencyId, Balance>) -> Self::Result {
		T::allowance(
			t.send.clone(),
			Location::Local(t.recv.clone()),
			FilterCurrency::Specific(t.id),
		)
		.is_ok() || T::allowance(
			t.send.clone(),
			Location::Local(t.recv.clone()),
			FilterCurrency::All,
		)
		.is_ok()
	}
}
pub struct PreLpTransfer<T>(sp_std::marker::PhantomData<T>);

impl<T: TransferAllowance<AccountId, CurrencyId = FilterCurrency, Location = Location>>
	PreConditions<(AccountId, DomainAddress, CurrencyId)> for PreLpTransfer<T>
{
	type Result = DispatchResult;

	fn check(t: (AccountId, DomainAddress, CurrencyId)) -> Self::Result {
		let (sender, receiver, currency) = t;
		T::allowance(
			sender.clone(),
			Location::Address(receiver.clone()),
			FilterCurrency::Specific(currency),
		)
		.or_else(|_| T::allowance(sender, Location::Address(receiver), FilterCurrency::All))
	}
}

#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, RuntimeDebugNoBound, Encode, Decode, TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct PreBalanceTransferExtension<T: frame_system::Config>(sp_std::marker::PhantomData<T>);

impl<T: frame_system::Config> PreBalanceTransferExtension<T> {
	pub fn new() -> Self {
		PreBalanceTransferExtension(sp_std::marker::PhantomData::default())
	}
}

impl<T> SignedExtension for PreBalanceTransferExtension<T>
where
	T: frame_system::Config<AccountId = AccountId>
		+ pallet_balances::Config
		+ pallet_transfer_allowlist::Config<CurrencyId = FilterCurrency, Location = Location>
		+ Sync
		+ Send,
	<T as frame_system::Config>::RuntimeCall: IsSubType<pallet_balances::Call<T>>,
{
	type AccountId = T::AccountId;
	type AdditionalSigned = ();
	type Call = T::RuntimeCall;
	type Pre = ();

	const IDENTIFIER: &'static str = "PreBalanceTransferExtension";

	fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
		Ok(())
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		_: &DispatchInfoOf<Self::Call>,
		_: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		let recv: T::AccountId = if let Some(call) =
			IsSubType::<pallet_balances::Call<T>>::is_sub_type(call)
		{
			match call {
				pallet_balances::Call::transfer { dest, .. }
				| pallet_balances::Call::transfer_all { dest, .. }
				| pallet_balances::Call::transfer_allow_death { dest, .. }
				| pallet_balances::Call::transfer_keep_alive { dest, .. } => {
					<T as frame_system::Config>::Lookup::lookup(dest.clone())
						.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Call))?
				}

				// If the call is not a transfer we are fine with it to go through without futher
				// checks
				_ => return Ok(()),
			}
		} else {
			return Ok(());
		};

		pallet_transfer_allowlist::pallet::Pallet::<T>::allowance(
			who.clone(),
			Location::Local(recv.clone()),
			FilterCurrency::All,
		)
		.or_else(|_| {
			pallet_transfer_allowlist::pallet::Pallet::<T>::allowance(
				who.clone(),
				Location::Local(recv.clone()),
				FilterCurrency::Specific(CurrencyId::Native),
			)
		})
		.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Custom(255)))
	}
}
