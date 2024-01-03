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

//! # Common types and primitives used for Centrifuge chain runtime.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod account_conversion;
pub mod apis;
pub mod changes;
pub mod evm;
pub mod fees;
pub mod gateway;
pub mod migrations;
pub mod oracle;
pub mod transfer_filter;
pub mod xcm;

use cfg_primitives::Balance;
use cfg_types::{fee_keys::FeeKey, tokens::CurrencyId};
use orml_traits::GetByKey;
use sp_core::parameter_types;
use sp_runtime::traits::Get;
use sp_std::marker::PhantomData;

parameter_types! {
	/// The native currency identifier of our currency id enum
	/// to be used for Get<CurrencyId> types.
	pub const NativeCurrency: CurrencyId = CurrencyId::Native;

	/// The hold identifier in our system to be used for
	/// Get<()> types
	pub const HoldId: HoldIdentifier = ();
}

pub struct AllowanceDeposit<T>(sp_std::marker::PhantomData<T>);
impl<T: cfg_traits::fees::Fees<Balance = Balance, FeeKey = FeeKey>> Get<Balance>
	for AllowanceDeposit<T>
{
	fn get() -> Balance {
		T::fee_value(FeeKey::AllowanceCreation)
	}
}

/// To be used with the transfer-allowlist pallet across runtimes
pub type HoldIdentifier = ();

#[macro_export]
macro_rules! production_or_benchmark {
	($production:expr, $benchmark:expr) => {{
		if cfg!(feature = "runtime-benchmarks") {
			$benchmark
		} else {
			$production
		}
	}};
}

pub struct CurrencyED<T>(PhantomData<T>);
impl<T> GetByKey<CurrencyId, Balance> for CurrencyED<T>
where
	T: pallet_balances::Config<Balance = Balance>
		+ orml_asset_registry::Config<AssetId = CurrencyId, Balance = Balance>,
{
	fn get(currency_id: &CurrencyId) -> Balance {
		match currency_id {
			CurrencyId::Native => T::ExistentialDeposit::get(),
			currency_id => orml_asset_registry::Pallet::<T>::metadata(currency_id)
				.map(|metadata| metadata.existential_deposit)
				.unwrap_or_default(),
		}
	}
}

pub mod xcm_fees {
	use cfg_primitives::{constants::currency_decimals, types::Balance};
	use frame_support::weights::constants::{ExtrinsicBaseWeight, WEIGHT_REF_TIME_PER_SECOND};

	// The fee cost per second for transferring the native token in cents.
	pub fn native_per_second() -> Balance {
		default_per_second(currency_decimals::NATIVE)
	}

	pub fn ksm_per_second() -> Balance {
		default_per_second(currency_decimals::KSM) / 50
	}

	pub fn default_per_second(decimals: u32) -> Balance {
		let base_weight = Balance::from(ExtrinsicBaseWeight::get().ref_time());
		let default_per_second = WEIGHT_REF_TIME_PER_SECOND as u128 / base_weight;
		default_per_second * base_fee(decimals)
	}

	fn base_fee(decimals: u32) -> Balance {
		dollar(decimals)
			// cents
			.saturating_div(100)
			// a tenth of a cent
			.saturating_div(10)
	}

	pub fn dollar(decimals: u32) -> Balance {
		10u128.saturating_pow(decimals)
	}
}

/// AssetRegistry's AssetProcessor
pub mod asset_registry {
	use cfg_primitives::types::{AccountId, Balance};
	use cfg_types::tokens::{CurrencyId, CustomMetadata};
	use frame_support::{
		dispatch::RawOrigin,
		sp_std::marker::PhantomData,
		traits::{EnsureOrigin, EnsureOriginWithArg},
	};
	use orml_traits::asset_registry::{AssetMetadata, AssetProcessor};
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
	use scale_info::TypeInfo;
	use sp_runtime::DispatchError;

	#[derive(
		Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
	)]
	pub struct CustomAssetProcessor;

	impl AssetProcessor<CurrencyId, AssetMetadata<Balance, CustomMetadata>> for CustomAssetProcessor {
		fn pre_register(
			id: Option<CurrencyId>,
			metadata: AssetMetadata<Balance, CustomMetadata>,
		) -> Result<(CurrencyId, AssetMetadata<Balance, CustomMetadata>), DispatchError> {
			match id {
				Some(id) => Ok((id, metadata)),
				None => Err(DispatchError::Other("asset-registry: AssetId is required")),
			}
		}

		fn post_register(
			_id: CurrencyId,
			_asset_metadata: AssetMetadata<Balance, CustomMetadata>,
		) -> Result<(), DispatchError> {
			Ok(())
		}
	}

	/// The OrmlAssetRegistry::AuthorityOrigin impl
	pub struct AuthorityOrigin<
		// The origin type
		Origin,
		// The default EnsureOrigin impl used to authorize all
		// assets besides tranche tokens.
		DefaultEnsureOrigin,
	>(PhantomData<(Origin, DefaultEnsureOrigin)>);

	impl<
			Origin: Into<Result<RawOrigin<AccountId>, Origin>> + From<RawOrigin<AccountId>>,
			DefaultEnsureOrigin: EnsureOrigin<Origin>,
		> EnsureOriginWithArg<Origin, Option<CurrencyId>> for AuthorityOrigin<Origin, DefaultEnsureOrigin>
	{
		type Success = ();

		fn try_origin(
			origin: Origin,
			asset_id: &Option<CurrencyId>,
		) -> Result<Self::Success, Origin> {
			match asset_id {
				// Only the pools pallet should directly register/update tranche tokens
				Some(CurrencyId::Tranche(_, _)) => Err(origin),

				// Any other `asset_id` defaults to EnsureRoot
				_ => DefaultEnsureOrigin::try_origin(origin).map(|_| ()),
			}
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn try_successful_origin(_asset_id: &Option<CurrencyId>) -> Result<Origin, ()> {
			Err(())
		}
	}
}

/// Module for investment portfolio common to all runtimes
pub mod investment_portfolios {
	use cfg_primitives::{Balance, PoolId, TrancheId};
	use cfg_traits::{
		investments::{InvestmentCollector, TrancheCurrency},
		PoolInspect, Seconds,
	};
	use cfg_types::{investments::InvestmentPortfolio, tokens::CurrencyId};
	use frame_support::traits::{
		fungibles,
		tokens::{Fortitude, Preservation},
	};
	use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

	/// Get the PoolId, CurrencyId, InvestmentId, and Balance for all
	/// investments for an account.
	///
	/// NOTE: Moving inner scope to any pallet would introduce tight(er)
	/// coupling due to requirement of iterating over storage maps which in turn
	/// require the pallet's Config trait.
	pub fn get_account_portfolio<T, PoolInspector>(
		investor: <T as frame_system::Config>::AccountId,
	) -> Vec<(
		<T as pallet_investments::Config>::InvestmentId,
		InvestmentPortfolio<Balance, CurrencyId>,
	)>
	where
		T: frame_system::Config
			+ pallet_investments::Config
			+ orml_tokens::Config
			+ pallet_restricted_tokens::Config,
		<T as pallet_investments::Config>::InvestmentId: TrancheCurrency<PoolId, TrancheId>
			+ Into<<T as orml_tokens::Config>::CurrencyId>
			+ Ord
			+ Into<<T as pallet_restricted_tokens::Config>::CurrencyId>,
		CurrencyId: From<<T as orml_tokens::Config>::CurrencyId>
			+ From<<T as pallet_restricted_tokens::Config>::CurrencyId>,
		<T as pallet_restricted_tokens::Config>::CurrencyId:
			From<<T as orml_tokens::Config>::CurrencyId>,
		Balance: From<<T as pallet_investments::Config>::Amount>
			+ From<<T as pallet_restricted_tokens::Config>::Balance>,
		PoolInspector: PoolInspect<
			<T as frame_system::Config>::AccountId,
			<T as pallet_restricted_tokens::Config>::CurrencyId,
			PoolId = PoolId,
			TrancheId = TrancheId,
			Moment = Seconds,
		>,
	{
		let mut portfolio = BTreeMap::<
			<T as pallet_investments::Config>::InvestmentId,
			InvestmentPortfolio<Balance, CurrencyId>,
		>::new();

		// Denote current tranche token balances before dry running collecting
		orml_tokens::Accounts::<T>::iter_key_prefix(&investor).for_each(|currency| {
			if let CurrencyId::Tranche(pool_id, tranche_id) = CurrencyId::from(currency) {
				let pool_currency = PoolInspector::currency_for(pool_id)
					.expect("Pool must exist; qed")
					.into();
				let free_balance = <pallet_restricted_tokens::Pallet<T> as fungibles::Inspect<
					T::AccountId,
				>>::reducible_balance(
					currency.into(),
					&investor,
					Preservation::Preserve,
					Fortitude::Polite,
				);
				let reserved_balance = <pallet_restricted_tokens::Pallet<T> as fungibles::InspectHold<
					T::AccountId,
				>>::balance_on_hold(currency.into(), &(), &investor);

				portfolio
					.entry(TrancheCurrency::generate(pool_id, tranche_id))
					.and_modify(|p| {
						p.free_tranche_tokens = free_balance.into();
						p.reserved_tranche_tokens = reserved_balance.into();
					})
					.or_insert(
						InvestmentPortfolio::<Balance, CurrencyId>::new(pool_currency)
							.with_free_tranche_tokens(free_balance.into())
							.with_reserved_tranche_tokens(reserved_balance.into()),
					);
			}
		});

		// Set pending invest currency and claimable tranche tokens
		pallet_investments::InvestOrders::<T>::iter_key_prefix(&investor).for_each(|invest_id| {
			let pool_currency =
				PoolInspector::currency_for(invest_id.of_pool()).expect("Pool must exist; qed");

			// Collect such that we can determine claimable tranche tokens
			// NOTE: Does not modify storage since RtAPI is readonly
			let _ =
				pallet_investments::Pallet::<T>::collect_investment(investor.clone(), invest_id);
			let amount = pallet_investments::InvestOrders::<T>::get(&investor, invest_id)
				.map(|order| order.amount())
				.unwrap_or_default();
			let free_tranche_tokens_new: Balance = <pallet_restricted_tokens::Pallet<T> as fungibles::Inspect<
				T::AccountId,
			>>::reducible_balance(
				invest_id.into(),
				&investor,
				Preservation::Preserve,
				Fortitude::Polite,
			).into();

			portfolio
				.entry(invest_id)
				.and_modify(|p| {
					p.pending_invest_currency = amount.into();
					if p.free_tranche_tokens < free_tranche_tokens_new {
						p.claimable_tranche_tokens =
							free_tranche_tokens_new.saturating_sub(p.free_tranche_tokens);
					}
				})
				.or_insert(
					InvestmentPortfolio::<Balance, CurrencyId>::new(pool_currency.into())
						.with_pending_invest_currency(amount.into())
						.with_claimable_tranche_tokens(free_tranche_tokens_new),
				);
		});

		// Set pending tranche tokens and claimable invest currency
		pallet_investments::RedeemOrders::<T>::iter_key_prefix(&investor).for_each(|invest_id| {
			let pool_currency =
				PoolInspector::currency_for(invest_id.of_pool()).expect("Pool must exist; qed");
			let balance_before: Balance =
					<pallet_restricted_tokens::Pallet<T> as fungibles::Inspect<
						T::AccountId,
					>>::reducible_balance(
						pool_currency,
						&investor,
						Preservation::Preserve,
						Fortitude::Polite,
					).into();

			// Collect such that we can determine claimable invest currency
			// NOTE: Does not modify storage since RtAPI is readonly
			let _ =
				pallet_investments::Pallet::<T>::collect_redemption(investor.clone(), invest_id);
			let amount = pallet_investments::RedeemOrders::<T>::get(&investor, invest_id)
				.map(|order| order.amount())
				.unwrap_or_default();
			let balance_after: Balance =
					<pallet_restricted_tokens::Pallet<T> as fungibles::Inspect<
						T::AccountId,
					>>::reducible_balance(
						pool_currency,
						&investor,
						Preservation::Preserve,
						Fortitude::Polite,
					).into();

			portfolio
				.entry(invest_id)
				.and_modify(|p| {
					p.pending_redeem_tranche_tokens = amount.into();
					if balance_before < balance_after {
						p.claimable_currency = balance_after.saturating_sub(balance_before);
					}
				})
				.or_insert(
					InvestmentPortfolio::<Balance, CurrencyId>::new(pool_currency.into())
						.with_pending_redeem_tranche_tokens(amount.into())
						.with_claimable_currency(balance_after),
				);
		});

		portfolio.into_iter().collect()
	}
}

pub mod xcm_transactor {
	use parity_scale_codec::{Decode, Encode};
	use scale_info::TypeInfo;
	use sp_std::{vec, vec::Vec};
	use xcm_primitives::{UtilityAvailableCalls, UtilityEncodeCall, XcmTransact};

	/// NOTE: our usage of XcmTransactor does NOT use this type so we have it
	/// implement the required traits by returning safe dummy values.
	#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, TypeInfo)]
	pub struct NullTransactor {}

	impl UtilityEncodeCall for NullTransactor {
		fn encode_call(self, _call: UtilityAvailableCalls) -> Vec<u8> {
			vec![]
		}
	}

	impl XcmTransact for NullTransactor {
		fn destination(self) -> xcm::latest::MultiLocation {
			Default::default()
		}
	}
}

pub mod foreign_investments {
	use cfg_primitives::{conversion::convert_balance_decimals, Balance};
	use cfg_traits::{
		ConversionFromAssetBalance, ConversionToAssetBalance, IdentityCurrencyConversion,
	};
	use cfg_types::tokens::CurrencyId;
	use frame_support::pallet_prelude::PhantomData;
	use orml_traits::asset_registry::Inspect;
	use sp_runtime::DispatchError;

	/// Simple currency converter which maps the amount of the outgoing currency
	/// to the precision of the incoming one. E.g., the worth of 100
	/// EthWrappedDai in USDC.
	///
	/// Requires currencies to have their decimal precision registered in an
	/// asset registry. Moreover, one of the currencies must be a allowed as
	/// pool currency.
	///
	/// NOTE: This converter is only supposed to be used short-term as an MVP
	/// for stable coin conversions. We assume those conversions to be 1-to-1
	/// bidirectionally. In the near future, this conversion must be improved to
	/// account for conversion ratios other than 1.0.
	pub struct IdentityPoolCurrencyConverter<AssetRegistry>(PhantomData<AssetRegistry>);

	impl<AssetRegistry> IdentityCurrencyConversion for IdentityPoolCurrencyConverter<AssetRegistry>
	where
		AssetRegistry: Inspect<
			AssetId = CurrencyId,
			Balance = Balance,
			CustomMetadata = cfg_types::tokens::CustomMetadata,
		>,
	{
		type Balance = Balance;
		type Currency = CurrencyId;
		type Error = DispatchError;

		fn stable_to_stable(
			currency_in: Self::Currency,
			currency_out: Self::Currency,
			amount_out: Self::Balance,
		) -> Result<Self::Balance, Self::Error> {
			match (currency_out, currency_in) {
				(from, to) if from == to => Ok(amount_out),
				(CurrencyId::ForeignAsset(_), CurrencyId::ForeignAsset(_)) => {
					let from_metadata = AssetRegistry::metadata(&currency_out)
						.ok_or(DispatchError::CannotLookup)?;
					let to_metadata =
						AssetRegistry::metadata(&currency_in).ok_or(DispatchError::CannotLookup)?;
					frame_support::ensure!(
						from_metadata.additional.pool_currency
							|| to_metadata.additional.pool_currency,
						DispatchError::Token(sp_runtime::TokenError::Unsupported)
					);

					convert_balance_decimals(
						from_metadata.decimals,
						to_metadata.decimals,
						amount_out,
					)
					.map_err(DispatchError::from)
				}
				_ => Err(DispatchError::Token(sp_runtime::TokenError::Unsupported)),
			}
		}
	}

	/// Provides means of applying the decimals of an incoming currency to the
	/// amount of an outgoing currency.
	///
	/// NOTE: Either the incoming (in case of `ConversionFromAssetBalance`) or
	/// outgoing currency (in case of `ConversionToAssetBalance`) is assumed
	/// to be `CurrencyId::Native`.
	pub struct NativeBalanceDecimalConverter<AssetRegistry>(PhantomData<AssetRegistry>);

	impl<AssetRegistry> ConversionToAssetBalance<Balance, CurrencyId, Balance>
		for NativeBalanceDecimalConverter<AssetRegistry>
	where
		AssetRegistry: Inspect<
			AssetId = CurrencyId,
			Balance = Balance,
			CustomMetadata = cfg_types::tokens::CustomMetadata,
		>,
	{
		type Error = DispatchError;

		fn to_asset_balance(
			balance: Balance,
			currency_in: CurrencyId,
		) -> Result<Balance, DispatchError> {
			match currency_in {
				CurrencyId::Native => Ok(balance),
				CurrencyId::ForeignAsset(_) => {
					let to_decimals = AssetRegistry::metadata(&currency_in)
						.ok_or(DispatchError::CannotLookup)?
						.decimals;
					convert_balance_decimals(
						cfg_primitives::currency_decimals::NATIVE,
						to_decimals,
						balance,
					)
					.map_err(DispatchError::from)
				}
				_ => Err(DispatchError::Token(sp_runtime::TokenError::Unsupported)),
			}
		}
	}

	impl<AssetRegistry> ConversionFromAssetBalance<Balance, CurrencyId, Balance>
		for NativeBalanceDecimalConverter<AssetRegistry>
	where
		AssetRegistry: Inspect<
			AssetId = CurrencyId,
			Balance = Balance,
			CustomMetadata = cfg_types::tokens::CustomMetadata,
		>,
	{
		type Error = DispatchError;

		fn from_asset_balance(
			balance: Balance,
			currency_out: CurrencyId,
		) -> Result<Balance, DispatchError> {
			match currency_out {
				CurrencyId::Native => Ok(balance),
				CurrencyId::ForeignAsset(_) => {
					let from_decimals = AssetRegistry::metadata(&currency_out)
						.ok_or(DispatchError::CannotLookup)?
						.decimals;
					convert_balance_decimals(
						from_decimals,
						cfg_primitives::currency_decimals::NATIVE,
						balance,
					)
					.map_err(DispatchError::from)
				}
				_ => Err(DispatchError::Token(sp_runtime::TokenError::Unsupported)),
			}
		}
	}
}

pub mod liquidity_pools {
	use cfg_primitives::{Balance, PoolId, TrancheId};
	use cfg_types::{domain_address::Domain, fixed_point::Ratio};

	pub type LiquidityPoolsMessage =
		pallet_liquidity_pools::Message<Domain, PoolId, TrancheId, Balance, Ratio>;
}

pub mod origin {
	use cfg_primitives::AccountId;
	use frame_support::traits::{EitherOfDiverse, SortedMembers};
	use frame_system::{EnsureRoot, EnsureSignedBy};
	use sp_core::Get;

	pub type EnsureAccountOrRoot<Account> =
		EitherOfDiverse<EnsureSignedBy<AdminOnly<Account>, AccountId>, EnsureRoot<AccountId>>;

	pub type EnsureAccountOrRootOr<Account, O> = EitherOfDiverse<EnsureAccountOrRoot<Account>, O>;

	pub struct AdminOnly<Account>(sp_std::marker::PhantomData<Account>);

	impl<Account> SortedMembers<AccountId> for AdminOnly<Account>
	where
		Account: Get<AccountId>,
	{
		fn sorted_members() -> sp_std::vec::Vec<AccountId> {
			sp_std::vec![Account::get()]
		}
	}

	#[cfg(test)]
	mod test {
		use cfg_primitives::HalfOfCouncil;
		use frame_support::traits::EnsureOrigin;
		use sp_core::{crypto::AccountId32, parameter_types};

		use super::*;

		parameter_types! {
			pub Admin: AccountId = AccountId::new([0u8;32]);
		}

		#[derive(Clone)]
		enum OuterOrigin {
			Raw(frame_system::RawOrigin<AccountId>),
			Council(pallet_collective::RawOrigin<AccountId, pallet_collective::Instance1>),
			Dummy,
		}

		impl Into<Result<frame_system::RawOrigin<AccountId>, OuterOrigin>> for OuterOrigin {
			fn into(self) -> Result<frame_system::RawOrigin<AccountId>, OuterOrigin> {
				match self {
					Self::Raw(raw) => Ok(raw),
					_ => Err(self),
				}
			}
		}

		impl
			Into<
				Result<
					pallet_collective::RawOrigin<
						sp_runtime::AccountId32,
						pallet_collective::Instance1,
					>,
					OuterOrigin,
				>,
			> for OuterOrigin
		{
			fn into(
				self,
			) -> Result<
				pallet_collective::RawOrigin<AccountId32, pallet_collective::Instance1>,
				OuterOrigin,
			> {
				match self {
					Self::Council(raw) => Ok(raw),
					_ => Err(self),
				}
			}
		}

		impl From<frame_system::RawOrigin<AccountId>> for OuterOrigin {
			fn from(value: frame_system::RawOrigin<AccountId>) -> Self {
				Self::Raw(value)
			}
		}

		impl From<pallet_collective::RawOrigin<AccountId, pallet_collective::Instance1>> for OuterOrigin {
			fn from(
				value: pallet_collective::RawOrigin<AccountId, pallet_collective::Instance1>,
			) -> Self {
				Self::Council(value)
			}
		}

		mod ensure_account_or_root_or {
			use super::*;

			#[test]
			fn works_with_account() {
				let origin = OuterOrigin::Raw(frame_system::RawOrigin::Signed(Admin::get()));

				assert!(
					EnsureAccountOrRootOr::<Admin, HalfOfCouncil>::ensure_origin(origin).is_ok()
				)
			}

			#[test]
			fn fails_with_non_admin_account() {
				let origin =
					OuterOrigin::Raw(frame_system::RawOrigin::Signed(AccountId::from([1u8; 32])));

				assert!(
					EnsureAccountOrRootOr::<Admin, HalfOfCouncil>::ensure_origin(origin).is_err()
				)
			}

			#[test]
			fn works_with_half_of_council() {
				let origin = OuterOrigin::Council(pallet_collective::RawOrigin::Members(5, 9));

				assert!(
					EnsureAccountOrRootOr::<Admin, HalfOfCouncil>::ensure_origin(origin).is_ok()
				)
			}

			#[test]
			fn fails_with_less_than_half_of_council() {
				let origin = OuterOrigin::Council(pallet_collective::RawOrigin::Members(4, 9));

				assert!(
					EnsureAccountOrRootOr::<Admin, HalfOfCouncil>::ensure_origin(origin).is_err()
				)
			}

			#[test]
			fn works_with_root() {
				let origin = OuterOrigin::Raw(frame_system::RawOrigin::Root);

				assert!(
					EnsureAccountOrRootOr::<Admin, HalfOfCouncil>::ensure_origin(origin).is_ok()
				)
			}

			#[test]
			fn fails_with_none() {
				let origin = OuterOrigin::Raw(frame_system::RawOrigin::None);

				assert!(
					EnsureAccountOrRootOr::<Admin, HalfOfCouncil>::ensure_origin(origin).is_err()
				)
			}

			#[test]
			fn fails_with_dummy() {
				let origin = OuterOrigin::Dummy;

				assert!(
					EnsureAccountOrRootOr::<Admin, HalfOfCouncil>::ensure_origin(origin).is_err()
				)
			}
		}

		mod ensure_account_or_root {
			use super::*;

			#[test]
			fn works_with_account() {
				let origin = OuterOrigin::Raw(frame_system::RawOrigin::Signed(Admin::get()));

				assert!(EnsureAccountOrRoot::<Admin>::ensure_origin(origin).is_ok())
			}

			#[test]
			fn fails_with_non_admin_account() {
				let origin =
					OuterOrigin::Raw(frame_system::RawOrigin::Signed(AccountId::from([1u8; 32])));

				assert!(EnsureAccountOrRoot::<Admin>::ensure_origin(origin).is_err())
			}

			#[test]
			fn works_with_root() {
				let origin = OuterOrigin::Raw(frame_system::RawOrigin::Root);

				assert!(EnsureAccountOrRoot::<Admin>::ensure_origin(origin).is_ok())
			}

			#[test]
			fn fails_with_none() {
				let origin = OuterOrigin::Raw(frame_system::RawOrigin::None);

				assert!(EnsureAccountOrRoot::<Admin>::ensure_origin(origin).is_err())
			}

			#[test]
			fn fails_with_dummy() {
				let origin = OuterOrigin::Dummy;

				assert!(EnsureAccountOrRoot::<Admin>::ensure_origin(origin).is_err())
			}
		}
	}
}

pub mod permissions {
	use cfg_primitives::{AccountId, PoolId};
	use cfg_traits::{Permissions, PreConditions};
	use cfg_types::{
		permissions::{PermissionScope, PoolRole, Role},
		tokens::CurrencyId,
	};
	use sp_std::marker::PhantomData;

	/// Check if an account has a pool admin role
	pub struct PoolAdminCheck<P>(PhantomData<P>);

	impl<P> PreConditions<(AccountId, PoolId)> for PoolAdminCheck<P>
	where
		P: Permissions<AccountId, Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>,
	{
		type Result = bool;

		fn check((account_id, pool_id): (AccountId, PoolId)) -> bool {
			P::has(
				PermissionScope::Pool(pool_id),
				account_id,
				Role::PoolRole(PoolRole::PoolAdmin),
			)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn satisfy((account_id, pool_id): (AccountId, PoolId)) {
			P::add(
				PermissionScope::Pool(pool_id),
				account_id,
				Role::PoolRole(PoolRole::PoolAdmin),
			)
			.unwrap();
		}
	}
}
