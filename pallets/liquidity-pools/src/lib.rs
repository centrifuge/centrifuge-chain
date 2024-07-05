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

//! # Liquidity Pools pallet
//!
//! Provides the toolset to enable foreign investments on foreign domains.
//!
//! - [`Pallet`]
//!
//! ## Assumptions
//! - Sending/recipient domains handle cross-chain transferred currencies
//!   properly on their side. This pallet only ensures correctness on the local
//!   domain.
//! - The implementer of the pallet's associated `ForeignInvestment` type sends
//!   notifications for completed investment decrements via the
//!   `DecreasedForeignInvestOrderHook`. Otherwise the domain which initially
//!   sent the `DecreaseInvestOrder` message will never be notified about the
//!   completion.
//! - The implementer of the pallet's associated `ForeignInvestment` type sends
//!   notifications for completed redemption collections via the
//!   `CollectedForeignRedemptionHook`. Otherwise the domain which initially
//!   sent the `CollectRedeem` message will never be notified about the
//!   completion.
//! - The pallet's associated `TreasuryAccount` holds sufficient balance for the
//!   corresponding fee currencies of all possible recipient domains for the
//!   following outgoing messages: [`Message::ExecutedDecreaseInvestOrder`],
//!   [`Message::ExecutedDecreaseRedeemOrder`],
//!   [`Message::ExecutedCollectInvest`], [`Message::ExecutedCollectRedeem`],
//!   [`Message::ScheduleUpgrade`].

#![cfg_attr(not(feature = "std"), no_std)]
use core::convert::TryFrom;

use cfg_traits::{
	liquidity_pools::{InboundQueue, OutboundQueue},
	PreConditions,
};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	tokens::GeneralCurrencyIndex,
};
use cfg_utils::vec_to_fixed_array;
use frame_support::{
	traits::{
		fungibles::{Inspect, Mutate},
		PalletInfo,
	},
	transactional,
};
use orml_traits::asset_registry::{self, Inspect as _};
pub use pallet::*;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Convert},
	FixedPointNumber, SaturatedConversion,
};
use sp_std::{convert::TryInto, vec};
use staging_xcm::{
	v4::{Junction::*, NetworkId},
	VersionedLocation,
};

// NOTE: Should be replaced with generated weights in the future. For now, let's
// be defensive.
pub mod defensive_weights;

/// Serializer for the LiquidityPool's Generic Message Passing Format (GMPF)
mod gmpf {
	mod de;
	mod error;
	mod ser;

	pub use de::from_slice;
	pub use ser::to_vec;
}

mod message;
pub use message::Message;

pub mod hooks;
mod inbound;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub type GeneralCurrencyIndexType = u128;

pub type GeneralCurrencyIndexOf<T> =
	GeneralCurrencyIndex<GeneralCurrencyIndexType, <T as pallet::Config>::GeneralCurrencyPrefix>;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{
		investments::{ForeignInvestment, TrancheCurrency},
		CurrencyInspect, Permissions, PoolInspect, Seconds, TimeAsSecs, TrancheTokenPrice,
	};
	use cfg_types::{
		permissions::{PermissionScope, PoolRole, Role},
		tokens::{CustomMetadata, LiquidityPoolsWrappedToken},
		EVMChainId,
	};
	use frame_support::{
		pallet_prelude::*,
		traits::tokens::{Fortitude, Precision, Preservation},
	};
	use frame_system::pallet_prelude::*;
	use parity_scale_codec::HasCompact;
	use sp_runtime::{traits::Zero, DispatchError};

	use super::*;
	use crate::defensive_weights::WeightInfo;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// The source of truth for the balance of accounts in native currency.
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ Into<u128>
			+ From<u128>;

		type PoolId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ core::fmt::Debug
			+ Into<u64>
			+ From<u64>;

		type TrancheId: Member
			+ Parameter
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ TypeInfo
			+ From<[u8; 16]>
			+ Into<[u8; 16]>;

		/// The fixed point number representation for higher precision.
		type BalanceRatio: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber<Inner = u128>
			+ TypeInfo;

		/// The source of truth for pool inspection operations such as its
		/// existence, the corresponding tranche token or the investment
		/// currency.
		type PoolInspect: PoolInspect<
			Self::AccountId,
			Self::CurrencyId,
			PoolId = Self::PoolId,
			TrancheId = Self::TrancheId,
		>;

		type TrancheTokenPrice: TrancheTokenPrice<
			Self::AccountId,
			Self::CurrencyId,
			BalanceRatio = Self::BalanceRatio,
			PoolId = Self::PoolId,
			TrancheId = Self::TrancheId,
			Moment = Seconds,
		>;

		/// The source of truth for investment permissions.
		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId>,
			Error = DispatchError,
		>;

		/// The UNIX timestamp provider type required for checking the validity
		/// of investments.
		type Time: TimeAsSecs;

		/// The type for handling transfers, burning and minting of
		/// multi-assets.
		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>;

		/// The currency type of investments.
		type TrancheCurrency: TrancheCurrency<Self::PoolId, Self::TrancheId>
			+ Into<Self::CurrencyId>
			+ Clone;

		/// Enables investing and redeeming into investment classes with foreign
		/// currencies.
		type ForeignInvestment: ForeignInvestment<
			Self::AccountId,
			Amount = Self::Balance,
			TrancheAmount = Self::Balance,
			CurrencyId = Self::CurrencyId,
			Error = DispatchError,
			InvestmentId = <Self as Config>::TrancheCurrency,
		>;

		/// The source of truth for the transferability of assets via the
		/// LiquidityPools feature.
		type AssetRegistry: asset_registry::Inspect<
			AssetId = Self::CurrencyId,
			Balance = <Self as Config>::Balance,
			CustomMetadata = CustomMetadata,
		>;

		/// The currency type of transferable tokens.
		type CurrencyId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen
			+ TryInto<GeneralCurrencyIndexOf<Self>, Error = DispatchError>
			+ TryFrom<GeneralCurrencyIndexOf<Self>, Error = DispatchError>
			// Enables checking whether currency is tranche token
			+ CurrencyInspect<CurrencyId = Self::CurrencyId>;

		/// The converter from a DomainAddress to a Substrate AccountId.
		type DomainAddressToAccountId: Convert<DomainAddress, Self::AccountId>;

		/// The converter from a Domain and a 32 byte array to DomainAddress.
		type DomainAccountToDomainAddress: Convert<(Domain, [u8; 32]), DomainAddress>;

		/// The type for processing outgoing messages.
		type OutboundQueue: OutboundQueue<
			Sender = Self::AccountId,
			Message = Message,
			Destination = Domain,
		>;

		/// The prefix for currencies added via the LiquidityPools feature.
		#[pallet::constant]
		type GeneralCurrencyPrefix: Get<[u8; 12]>;

		/// The type for paying the transaction fees for the dispatch of
		/// `Executed*` and `ScheduleUpgrade` messages.
		///
		/// NOTE: We need to make sure to collect the appropriate amount
		/// beforehand as part of receiving the corresponding investment
		/// message.
		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		type PreTransferFilter: PreConditions<
			(Self::AccountId, DomainAddress, Self::CurrencyId),
			Result = DispatchResult,
		>;

		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	#[allow(clippy::large_enum_variant)]
	pub enum Event<T: Config> {
		/// An incoming LP message was
		/// detected and is further processed
		IncomingMessage {
			sender: DomainAddress,
			message: Message,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to map the asset to the corresponding LiquidityPools' General
		/// Index representation and thus cannot be used as an
		/// investment currency.
		AssetNotFound,
		/// The metadata of the given asset does not declare it as a pool
		/// currency and thus it cannot be used as an investment currency.
		AssetMetadataNotPoolCurrency,
		/// The metadata of the given asset does not declare it as transferable
		/// via LiquidityPools'.
		AssetNotLiquidityPoolsTransferable,
		/// The asset is not a [LiquidityPoolsWrappedToken] and thus cannot be
		/// transferred via liquidity pools.
		AssetNotLiquidityPoolsWrappedToken,
		/// A pool could not be found.
		PoolNotFound,
		/// A tranche could not be found.
		TrancheNotFound,
		/// Could not find the metadata of a tranche token.
		TrancheMetadataNotFound,
		/// Failed to fetch a tranche token price.
		/// This can occur if `TrancheNotFound` or if effectively
		/// the price for this tranche has not yet been set.
		MissingTranchePrice,
		/// Transfer amount must be non-zero.
		InvalidTransferAmount,
		/// Senders balance is insufficient for transfer amount
		BalanceTooLow,
		/// A transfer to a non-whitelisted destination was attempted.
		UnauthorizedTransfer,
		/// Failed to decode an incoming message.
		InvalidIncomingMessage,
		/// The destination domain is invalid.
		InvalidDomain,
		/// The validity is in the past.
		InvalidTrancheInvestorValidity,
		/// The currency is not allowed to be transferred via LiquidityPools.
		InvalidTransferCurrency,
		/// The account derived from the [Domain] and [DomainAddress] has not
		/// been whitelisted as a TrancheInvestor.
		InvestorDomainAddressNotAMember,
		/// Only the PoolAdmin can execute a given operation.
		NotPoolAdmin,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as frame_system::Config>::AccountId: From<[u8; 32]> + Into<[u8; 32]>,
	{
		/// Add a pool to a given domain
		#[pallet::weight(T::WeightInfo::add_pool())]
		#[pallet::call_index(2)]
		pub fn add_pool(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			domain: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);

			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				Error::<T>::NotPoolAdmin
			);

			T::OutboundQueue::submit(
				who,
				domain,
				Message::AddPool {
					pool_id: pool_id.into(),
				},
			)?;
			Ok(())
		}

		/// Add a tranche to a given domain
		#[pallet::weight(T::WeightInfo::add_tranche())]
		#[pallet::call_index(3)]
		pub fn add_tranche(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			domain: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				Error::<T>::NotPoolAdmin
			);

			// Look up the metadata of the tranche token
			let investment_id = Self::derive_invest_id(pool_id, tranche_id)?;
			let metadata = T::AssetRegistry::metadata(&investment_id.into())
				.ok_or(Error::<T>::TrancheMetadataNotFound)?;
			let token_name = vec_to_fixed_array(metadata.name);
			let token_symbol = vec_to_fixed_array(metadata.symbol);

			// Send the message to the domain
			T::OutboundQueue::submit(
				who,
				domain,
				Message::AddTranche {
					pool_id: pool_id.into(),
					tranche_id: tranche_id.into(),
					decimals: metadata.decimals.saturated_into(),
					token_name,
					token_symbol,
					// NOTE: This value is for now intentionally hardcoded to 1 since that's the
					// only available option. We will design a dynamic approach going forward where
					// this value can be set on a per-tranche-token basis on storage.
					restriction_set: 1,
				},
			)?;

			Ok(())
		}

		/// Update the price of a tranche token.
		///
		/// By ensuring that registered currency location matches the specified
		/// domain, this call origin can be permissionless.
		///
		/// The `currency_id` parameter is necessary for the EVM side.
		#[pallet::weight(T::WeightInfo::update_token_price())]
		#[pallet::call_index(4)]
		pub fn update_token_price(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			currency_id: T::CurrencyId,
			destination: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// TODO(future): Once we diverge from 1-to-1 conversions for foreign and pool
			// currencies, this price must be first converted into the currency_id and then
			// re-denominated to 18 decimals (i.e. `Ratio` precision)
			let price_value = T::TrancheTokenPrice::get(pool_id, tranche_id)
				.ok_or(Error::<T>::MissingTranchePrice)?;

			// Check that the registered asset location matches the destination
			match Self::try_get_wrapped_token(&currency_id)? {
				LiquidityPoolsWrappedToken::EVM { chain_id, .. } => {
					ensure!(
						Domain::EVM(chain_id) == destination,
						Error::<T>::InvalidDomain
					);
				}
			}
			let currency = Self::try_get_general_index(currency_id)?;

			T::OutboundQueue::submit(
				who,
				destination,
				Message::UpdateTrancheTokenPrice {
					pool_id: pool_id.into(),
					tranche_id: tranche_id.into(),
					currency,
					price: price_value.price.into_inner(),
					computed_at: price_value.last_updated,
				},
			)?;

			Ok(())
		}

		/// Update a member
		#[pallet::weight(T::WeightInfo::update_member())]
		#[pallet::call_index(5)]
		pub fn update_member(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			domain_address: DomainAddress,
			valid_until: Seconds,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			ensure!(
				T::PoolInspect::tranche_exists(pool_id, tranche_id),
				Error::<T>::TrancheNotFound
			);
			ensure!(
				valid_until > T::Time::now(),
				Error::<T>::InvalidTrancheInvestorValidity
			);

			// Ensure that the destination address has been whitelisted as a TrancheInvestor
			// beforehand.
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					T::DomainAddressToAccountId::convert(domain_address.clone()),
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, valid_until))
				),
				Error::<T>::InvestorDomainAddressNotAMember
			);

			T::OutboundQueue::submit(
				who,
				domain_address.domain(),
				Message::UpdateMember {
					pool_id: pool_id.into(),
					tranche_id: tranche_id.into(),
					valid_until,
					member: domain_address.address(),
				},
			)?;

			Ok(())
		}

		/// Transfer tranche tokens to a given address.
		///
		/// NOTE: Assumes `OutboundQueue` to check whether destination is local.
		///
		/// NOTE: The transferring account is not kept alive as we allow its
		/// death.
		#[pallet::weight(T::WeightInfo::transfer())]
		#[pallet::call_index(6)]
		pub fn transfer_tranche_tokens(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			domain_address: DomainAddress,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					T::DomainAddressToAccountId::convert(domain_address.clone()),
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, T::Time::now()))
				),
				Error::<T>::UnauthorizedTransfer
			);

			// Ensure pool and tranche exist and derive invest id
			let invest_id = Self::derive_invest_id(pool_id, tranche_id)?;

			T::PreTransferFilter::check((
				who.clone(),
				domain_address.clone(),
				invest_id.clone().into(),
			))?;

			// Transfer to the domain account for bookkeeping
			T::Tokens::transfer(
				invest_id.into(),
				&who,
				&domain_address.domain().into_account(),
				amount,
				// NOTE: Here, we allow death
				Preservation::Expendable,
			)?;

			T::OutboundQueue::submit(
				who.clone(),
				domain_address.domain(),
				Message::TransferTrancheTokens {
					pool_id: pool_id.into(),
					tranche_id: tranche_id.into(),
					amount: amount.into(),
					domain: domain_address.domain().into(),
					sender: who.into(),
					receiver: domain_address.address(),
				},
			)?;

			Ok(())
		}

		/// Transfer non-tranche tokens to a given address.
		///
		/// NOTE: Assumes `OutboundQueue` to check whether destination is local.
		///
		/// NOTE: The transferring account is not kept alive as we allow its
		/// death.
		#[pallet::weight(T::WeightInfo::transfer())]
		#[pallet::call_index(7)]
		pub fn transfer(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			receiver: DomainAddress,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);
			ensure!(
				!T::CurrencyId::is_tranche_token(currency_id),
				Error::<T>::InvalidTransferCurrency
			);
			let currency = Self::try_get_general_index(currency_id)?;

			// Check that the registered asset location matches the destination
			match Self::try_get_wrapped_token(&currency_id)? {
				LiquidityPoolsWrappedToken::EVM { chain_id, .. } => {
					ensure!(
						Domain::EVM(chain_id) == receiver.domain(),
						Error::<T>::InvalidDomain
					);
				}
			}

			T::PreTransferFilter::check((who.clone(), receiver.clone(), currency_id))?;

			// NOTE: This check is needed as `burn_from` has not a good error resolution and
			//       might return `Arithmetic` errors.
			ensure!(
				T::Tokens::reducible_balance(
					currency_id,
					&who,
					Preservation::Expendable,
					// NOTE: We do not know whether there are locks or so, so we are using user
					//       privilege
					Fortitude::Polite
				) >= amount,
				Error::<T>::BalanceTooLow
			);

			// Burn token as we are never the reserve for LP tokens that are not tranche
			// tokens.
			T::Tokens::burn_from(
				currency_id,
				&who,
				amount,
				Precision::Exact,
				// NOTE: We do not know whether there are locks or so, so we are using user
				//       privilege
				Fortitude::Polite,
			)?;

			T::OutboundQueue::submit(
				who.clone(),
				receiver.domain(),
				Message::Transfer {
					amount: amount.into(),
					currency,
					sender: who.into(),
					receiver: receiver.address(),
				},
			)?;

			Ok(())
		}

		/// Add a currency to the set of known currencies on the domain derived
		/// from the given currency.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
		#[pallet::call_index(8)]
		pub fn add_currency(origin: OriginFor<T>, currency_id: T::CurrencyId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let currency = Self::try_get_general_index(currency_id)?;

			let LiquidityPoolsWrappedToken::EVM {
				chain_id,
				address: evm_address,
			} = Self::try_get_wrapped_token(&currency_id)?;

			T::OutboundQueue::submit(
				who,
				Domain::EVM(chain_id),
				Message::AddCurrency {
					currency,
					evm_address,
				},
			)?;

			Ok(())
		}

		/// Allow a currency to be used as a pool currency and to invest in a
		/// pool on the domain derived from the given currency.
		#[pallet::call_index(9)]
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
		pub fn allow_investment_currency(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			currency_id: T::CurrencyId,
		) -> DispatchResult {
			// TODO(future): In the future, should be permissioned by trait which
			// does not exist yet.
			// See spec: https://centrifuge.hackmd.io/SERpps-URlG4hkOyyS94-w?view#fn-add_pool_currency
			let who = ensure_signed(origin)?;

			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				Error::<T>::NotPoolAdmin
			);

			let (currency, chain_id) = Self::validate_investment_currency(currency_id)?;

			T::OutboundQueue::submit(
				who,
				Domain::EVM(chain_id),
				Message::AllowInvestmentCurrency {
					pool_id: pool_id.into(),
					currency,
				},
			)?;

			Ok(())
		}

		/// Schedule an upgrade of an EVM-based liquidity pool contract instance
		#[pallet::weight(<T as Config>::WeightInfo::schedule_upgrade())]
		#[pallet::call_index(10)]
		pub fn schedule_upgrade(
			origin: OriginFor<T>,
			evm_chain_id: EVMChainId,
			contract: [u8; 20],
		) -> DispatchResult {
			ensure_root(origin)?;

			T::OutboundQueue::submit(
				T::TreasuryAccount::get(),
				Domain::EVM(evm_chain_id),
				Message::ScheduleUpgrade { contract },
			)
		}

		/// Schedule an upgrade of an EVM-based liquidity pool contract instance
		#[pallet::weight(T::WeightInfo::cancel_upgrade())]
		#[pallet::call_index(11)]
		pub fn cancel_upgrade(
			origin: OriginFor<T>,
			evm_chain_id: EVMChainId,
			contract: [u8; 20],
		) -> DispatchResult {
			ensure_root(origin)?;

			T::OutboundQueue::submit(
				T::TreasuryAccount::get(),
				Domain::EVM(evm_chain_id),
				Message::CancelUpgrade { contract },
			)
		}

		/// Update the tranche token name and symbol on the specified domain
		///
		/// NOTE: Pulls the metadata from the `AssetRegistry` and thus requires
		/// the pool admin to have updated the tranche tokens metadata there
		/// beforehand.
		#[pallet::weight(T::WeightInfo::update_tranche_token_metadata())]
		#[pallet::call_index(12)]
		pub fn update_tranche_token_metadata(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			domain: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				T::PoolInspect::tranche_exists(pool_id, tranche_id),
				Error::<T>::TrancheNotFound
			);

			let investment_id = Self::derive_invest_id(pool_id, tranche_id)?;
			let metadata = T::AssetRegistry::metadata(&investment_id.into())
				.ok_or(Error::<T>::TrancheMetadataNotFound)?;
			let token_name = vec_to_fixed_array(metadata.name);
			let token_symbol = vec_to_fixed_array(metadata.symbol);

			T::OutboundQueue::submit(
				who,
				domain,
				Message::UpdateTrancheTokenMetadata {
					pool_id: pool_id.into(),
					tranche_id: tranche_id.into(),
					token_name,
					token_symbol,
				},
			)
		}

		/// Disallow a currency to be used as a pool currency and to invest in a
		/// pool on the domain derived from the given currency.
		#[pallet::call_index(13)]
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
		pub fn disallow_investment_currency(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			currency_id: T::CurrencyId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				Error::<T>::NotPoolAdmin
			);

			let (currency, chain_id) = Self::validate_investment_currency(currency_id)?;

			T::OutboundQueue::submit(
				who,
				Domain::EVM(chain_id),
				Message::DisallowInvestmentCurrency {
					pool_id: pool_id.into(),
					currency,
				},
			)?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Returns the `u128` general index of a currency as the concatenation
		/// of the configured `GeneralCurrencyPrefix` and its local currency
		/// identifier.
		///
		/// Requires the currency to be registered in the `AssetRegistry`.
		///
		/// NOTE: Reverse operation of `try_get_currency_id`.
		pub fn try_get_general_index(currency: T::CurrencyId) -> Result<u128, DispatchError> {
			ensure!(
				T::AssetRegistry::metadata(&currency).is_some(),
				Error::<T>::AssetNotFound
			);

			let general_index: GeneralCurrencyIndexOf<T> = T::CurrencyId::try_into(currency)?;

			Ok(general_index.index)
		}

		/// Returns the local currency identifier from from its general index.
		///
		/// Requires the currency to be registered in the `AssetRegistry`.
		///
		/// NOTE: Reverse operation of `try_get_general_index`.
		pub fn try_get_currency_id(
			index: GeneralCurrencyIndexOf<T>,
		) -> Result<T::CurrencyId, DispatchError> {
			let currency = T::CurrencyId::try_from(index)?;
			ensure!(
				T::AssetRegistry::metadata(&currency).is_some(),
				Error::<T>::AssetNotFound
			);

			Ok(currency)
		}

		/// Checks whether the given currency is transferable via LiquidityPools
		/// and whether its metadata contains a location which can be
		/// converted to [LiquidityPoolsWrappedToken].
		///
		/// Requires the currency to be registered in the `AssetRegistry`.
		pub fn try_get_wrapped_token(
			currency_id: &T::CurrencyId,
		) -> Result<LiquidityPoolsWrappedToken, DispatchError> {
			let meta = T::AssetRegistry::metadata(currency_id).ok_or(Error::<T>::AssetNotFound)?;
			ensure!(
				meta.additional.transferability.includes_liquidity_pools(),
				Error::<T>::AssetNotLiquidityPoolsTransferable
			);

			// We need to still support v3 until orml_asset_registry migrates to the last
			// version.
			let location = match meta.location {
				Some(VersionedLocation::V3(location)) => location.try_into().map_err(|_| {
					DispatchError::Other("v3 is isometric to v4 and should not fail")
				})?,
				Some(VersionedLocation::V4(location)) => location,
				_ => Err(Error::<T>::AssetNotLiquidityPoolsWrappedToken)?,
			};

			let pallet_index = <T as frame_system::Config>::PalletInfo::index::<Pallet<T>>();

			match location.unpack() {
				(
					0,
					&[PalletInstance(pallet_instance), GlobalConsensus(NetworkId::Ethereum { chain_id }), AccountKey20 {
						network: None,
						key: address,
					}],
				) if Some(pallet_instance.into()) == pallet_index => {
					Ok(LiquidityPoolsWrappedToken::EVM { chain_id, address })
				}
				_ => Err(Error::<T>::AssetNotLiquidityPoolsWrappedToken.into()),
			}
		}

		/// Ensures that the given pool and tranche exists and returns the
		/// corresponding investment id.
		pub fn derive_invest_id(
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
		) -> Result<<T as pallet::Config>::TrancheCurrency, DispatchError> {
			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			ensure!(
				T::PoolInspect::tranche_exists(pool_id, tranche_id),
				Error::<T>::TrancheNotFound
			);

			Ok(TrancheCurrency::generate(pool_id, tranche_id))
		}

		/// Performs multiple checks for the provided currency and returns its
		/// general index and the EVM chain ID associated with it.
		pub fn validate_investment_currency(
			currency_id: T::CurrencyId,
		) -> Result<(u128, EVMChainId), DispatchError> {
			// Ensure the currency is enabled as pool_currency
			let metadata =
				T::AssetRegistry::metadata(&currency_id).ok_or(Error::<T>::AssetNotFound)?;
			ensure!(
				metadata.additional.pool_currency,
				Error::<T>::AssetMetadataNotPoolCurrency
			);

			let currency = Self::try_get_general_index(currency_id)?;

			let LiquidityPoolsWrappedToken::EVM { chain_id, .. } =
				Self::try_get_wrapped_token(&currency_id)?;

			Ok((currency, chain_id))
		}

		fn domain_account_to_account_id(domain_account: (Domain, [u8; 32])) -> T::AccountId {
			let domain_address = T::DomainAccountToDomainAddress::convert(domain_account);
			T::DomainAddressToAccountId::convert(domain_address)
		}
	}

	impl<T: Config> InboundQueue for Pallet<T>
	where
		<T as frame_system::Config>::AccountId: From<[u8; 32]> + Into<[u8; 32]>,
	{
		type Message = Message;
		type Sender = DomainAddress;

		#[transactional]
		fn submit(sender: DomainAddress, msg: Message) -> DispatchResult {
			Self::deposit_event(Event::<T>::IncomingMessage {
				sender: sender.clone(),
				message: msg.clone(),
			});

			match msg {
				Message::Transfer {
					currency,
					receiver,
					amount,
					..
				} => Self::handle_transfer(currency.into(), receiver.into(), amount.into()),
				Message::TransferTrancheTokens {
					pool_id,
					tranche_id,
					domain,
					receiver,
					amount,
					..
				} => Self::handle_tranche_tokens_transfer(
					pool_id.into(),
					tranche_id.into(),
					sender.domain(),
					T::DomainAccountToDomainAddress::convert((domain.try_into()?, receiver)),
					amount.into(),
				),
				Message::IncreaseInvestOrder {
					pool_id,
					tranche_id,
					investor,
					currency,
					amount,
				} => Self::handle_increase_invest_order(
					pool_id.into(),
					tranche_id.into(),
					Self::domain_account_to_account_id((sender.domain(), investor)),
					currency.into(),
					amount.into(),
				),
				Message::DecreaseInvestOrder {
					pool_id,
					tranche_id,
					investor,
					currency,
					amount,
				} => Self::handle_decrease_invest_order(
					pool_id.into(),
					tranche_id.into(),
					Self::domain_account_to_account_id((sender.domain(), investor)),
					currency.into(),
					amount.into(),
				),
				Message::IncreaseRedeemOrder {
					pool_id,
					tranche_id,
					investor,
					amount,
					currency,
				} => Self::handle_increase_redeem_order(
					pool_id.into(),
					tranche_id.into(),
					Self::domain_account_to_account_id((sender.domain(), investor)),
					amount.into(),
					currency.into(),
					sender,
				),
				Message::DecreaseRedeemOrder {
					pool_id,
					tranche_id,
					investor,
					currency,
					amount,
				} => Self::handle_decrease_redeem_order(
					pool_id.into(),
					tranche_id.into(),
					Self::domain_account_to_account_id((sender.domain(), investor)),
					amount.into(),
					currency.into(),
					sender,
				),
				Message::CollectInvest {
					pool_id,
					tranche_id,
					investor,
					currency,
				} => Self::handle_collect_investment(
					pool_id.into(),
					tranche_id.into(),
					Self::domain_account_to_account_id((sender.domain(), investor)),
					currency.into(),
				),
				Message::CollectRedeem {
					pool_id,
					tranche_id,
					investor,
					currency,
				} => Self::handle_collect_redemption(
					pool_id.into(),
					tranche_id.into(),
					Self::domain_account_to_account_id((sender.domain(), investor)),
					currency.into(),
				),
				Message::CancelInvestOrder {
					pool_id,
					tranche_id,
					investor,
					currency,
				} => Self::handle_cancel_invest_order(
					pool_id.into(),
					tranche_id.into(),
					Self::domain_account_to_account_id((sender.domain(), investor)),
					currency.into(),
				),
				Message::CancelRedeemOrder {
					pool_id,
					tranche_id,
					investor,
					currency,
				} => Self::handle_cancel_redeem_order(
					pool_id.into(),
					tranche_id.into(),
					Self::domain_account_to_account_id((sender.domain(), investor)),
					currency.into(),
					sender,
				),
				_ => Err(Error::<T>::InvalidIncomingMessage.into()),
			}?;

			Ok(())
		}
	}
}
