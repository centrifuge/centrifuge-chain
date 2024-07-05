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

use cfg_primitives::Balance;
use cfg_types::{
	fee_keys::FeeKey,
	pools::PoolNav,
	tokens::{CurrencyId, StakingCurrency},
};
use orml_traits::GetByKey;
use pallet_loans::entities::input::PriceCollectionInput;
use pallet_pool_system::Nav;
use sp_core::parameter_types;
use sp_runtime::{
	traits::{Get, Zero},
	DispatchError,
};
use sp_std::marker::PhantomData;

pub mod account_conversion;
pub mod apis;
pub mod changes;
pub mod evm;
pub mod fees;
pub mod gateway;
pub mod migrations;
pub mod oracle;
pub mod origins;
pub mod pool;
pub mod remarks;
pub mod transfer_filter;
pub mod xcm;

pub mod instances {
	/// The rewards associated to block rewards
	pub type BlockRewards = pallet_rewards::Instance1;

	/// The technical fellowship collective which can whitelist proposal for the
	/// WhitelistedCaller track
	pub type TechnicalCollective = pallet_collective::Instance2;

	/// The technical membership which handles membership of the
	/// TechnicalCollective. It is not linked to the WhitelistedCaller track.
	pub type TechnicalMembership = pallet_membership::Instance1;

	/// The council collective which is used in Gov1.
	///
	/// NOTE: Will be deprecated once we have fully transitioned to OpenGov.
	pub type CouncilCollective = pallet_collective::Instance1;
}

parameter_types! {
	/// The native currency identifier of our currency id enum
	/// to be used for Get<CurrencyId> types.
	pub const NativeCurrency: CurrencyId = CurrencyId::Native;
}

pub struct AllowanceDeposit<T>(sp_std::marker::PhantomData<T>);
impl<T: cfg_traits::fees::Fees<Balance = Balance, FeeKey = FeeKey>> Get<Balance>
	for AllowanceDeposit<T>
{
	fn get() -> Balance {
		T::fee_value(FeeKey::AllowanceCreation)
	}
}

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
		+ orml_asset_registry::module::Config<AssetId = CurrencyId, Balance = Balance>,
{
	fn get(currency_id: &CurrencyId) -> Balance {
		match currency_id {
			CurrencyId::Native => T::ExistentialDeposit::get(),
			CurrencyId::Staking(StakingCurrency::BlockRewards) => T::ExistentialDeposit::get(),
			currency_id => orml_asset_registry::module::Pallet::<T>::metadata(currency_id)
				.map(|metadata| metadata.existential_deposit)
				.unwrap_or_default(),
		}
	}
}

pub fn update_nav<T>(
	pool_id: <T as pallet_pool_system::Config>::PoolId,
) -> Result<PoolNav<<T as pallet_pool_system::Config>::Balance>, DispatchError>
where
	T: pallet_loans::Config<
			PoolId = <T as pallet_pool_system::Config>::PoolId,
			Balance = <T as pallet_pool_system::Config>::Balance,
		> + pallet_pool_system::Config
		+ pallet_pool_fees::Config<
			PoolId = <T as pallet_pool_system::Config>::PoolId,
			Balance = <T as pallet_pool_system::Config>::Balance,
		>,
{
	let input_prices: PriceCollectionInput<T> =
		match pallet_loans::Pallet::<T>::registered_prices(pool_id) {
			Ok(_) => PriceCollectionInput::FromRegistry,
			Err(_) => PriceCollectionInput::Empty,
		};

	update_nav_with_input::<T>(pool_id, input_prices)
}

/// ## Updates the nav for a pool.
///
/// NOTE: Should NEVER be used in consensus relevant state changes!
///
/// ### Execution infos
/// * For external assets it is either using the latest
/// oracle prices if they are not outdated or it is using no prices and allows
/// the chain to use the estimates based on the linear accrual of the last
/// settlement prices.
/// * IF `nav_fees > nav_loans` then the `nav_total` will saturate at 0
pub fn update_nav_with_input<T>(
	pool_id: <T as pallet_pool_system::Config>::PoolId,
	price_input: PriceCollectionInput<T>,
) -> Result<PoolNav<<T as pallet_pool_system::Config>::Balance>, DispatchError>
where
	T: pallet_loans::Config<
			PoolId = <T as pallet_pool_system::Config>::PoolId,
			Balance = <T as pallet_pool_system::Config>::Balance,
		> + pallet_pool_system::Config
		+ pallet_pool_fees::Config<
			PoolId = <T as pallet_pool_system::Config>::PoolId,
			Balance = <T as pallet_pool_system::Config>::Balance,
		>,
{
	let mut pool = pallet_pool_system::Pool::<T>::get(pool_id)
		.ok_or(pallet_pool_system::Error::<T>::NoSuchPool)?;

	let prev_nav_loans = pallet_loans::Pallet::<T>::portfolio_valuation(pool_id).value();
	let nav_loans =
		pallet_loans::Pallet::<T>::update_portfolio_valuation_for_pool(pool_id, price_input)
			.map(|(nav_loans, _)| nav_loans)
			.unwrap_or(prev_nav_loans);

	let prev_fees_loans = pallet_pool_fees::Pallet::<T>::portfolio_valuation(pool_id).value();
	let nav_fees = pallet_pool_fees::Pallet::<T>::update_portfolio_valuation_for_pool(
		pool_id,
		&mut pool.reserve.total,
	)
	.map(|(nav_fees, _)| nav_fees)
	.unwrap_or(prev_fees_loans);

	let nav = Nav::new(nav_loans, nav_fees);
	let total = nav
		.total(pool.reserve.total)
		.unwrap_or(<T as pallet_pool_system::Config>::Balance::zero());

	Ok(PoolNav {
		nav_aum: nav.nav_aum,
		nav_fees: nav.nav_fees,
		reserve: pool.reserve.total,
		total,
	})
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
	use cfg_primitives::types::AccountId;
	use cfg_types::tokens::{AssetMetadata, CurrencyId};
	use frame_support::{
		dispatch::RawOrigin,
		traits::{EnsureOrigin, EnsureOriginWithArg},
	};
	use orml_traits::asset_registry::AssetProcessor;
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
	use scale_info::TypeInfo;
	use sp_runtime::DispatchError;
	use sp_std::marker::PhantomData;

	#[derive(
		Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
	)]
	pub struct CustomAssetProcessor;

	impl AssetProcessor<CurrencyId, AssetMetadata> for CustomAssetProcessor {
		fn pre_register(
			id: Option<CurrencyId>,
			metadata: AssetMetadata,
		) -> Result<(CurrencyId, AssetMetadata), DispatchError> {
			match id {
				Some(id) => Ok((id, metadata)),
				None => Err(DispatchError::Other("asset-registry: AssetId is required")),
			}
		}

		fn post_register(
			_id: CurrencyId,
			_asset_metadata: AssetMetadata,
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
		> EnsureOriginWithArg<Origin, Option<CurrencyId>>
		for AuthorityOrigin<Origin, DefaultEnsureOrigin>
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
	use cfg_primitives::{AccountId, Balance, PoolId};
	use cfg_traits::{
		investments::{InvestmentCollector, TrancheCurrency as _},
		PoolInspect,
	};
	use cfg_types::{
		investments::InvestmentPortfolio,
		tokens::{CurrencyId, TrancheCurrency},
	};
	use frame_support::traits::{
		fungibles,
		tokens::{Fortitude, Preservation},
	};
	use fungibles::{Inspect, InspectHold};
	use sp_runtime::DispatchError;
	use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

	/// Get the PoolId, CurrencyId, InvestmentId, and Balance for all
	/// investments for an account.
	///
	/// NOTE: Moving inner scope to any pallet would introduce tight(er)
	/// coupling due to requirement of iterating over storage maps which in turn
	/// require the pallet's Config trait.
	#[allow(clippy::type_complexity)]
	pub fn get_account_portfolio<T>(
		investor: AccountId,
	) -> Result<Vec<(TrancheCurrency, InvestmentPortfolio<Balance, CurrencyId>)>, DispatchError>
	where
		T: frame_system::Config<AccountId = AccountId>
			+ pallet_investments::Config<InvestmentId = TrancheCurrency, Amount = Balance>
			+ orml_tokens::Config<Balance = Balance, CurrencyId = CurrencyId>
			+ pallet_restricted_tokens::Config<Balance = Balance, CurrencyId = CurrencyId>
			+ pallet_pool_system::Config<PoolId = PoolId, CurrencyId = CurrencyId>,
	{
		let mut portfolio =
			BTreeMap::<TrancheCurrency, InvestmentPortfolio<Balance, CurrencyId>>::new();

		// Denote current tranche token balances before dry running collecting
		for currency in orml_tokens::Accounts::<T>::iter_key_prefix(&investor) {
			if let CurrencyId::Tranche(pool_id, tranche_id) = currency {
				let pool_currency = pallet_pool_system::Pallet::<T>::currency_for(pool_id)
					.ok_or(DispatchError::Other("Pool must exist; qed"))?;

				let free_balance = pallet_restricted_tokens::Pallet::<T>::reducible_balance(
					currency,
					&investor,
					Preservation::Preserve,
					Fortitude::Polite,
				);
				let reserved_balance = pallet_restricted_tokens::Pallet::<T>::balance_on_hold(
					currency,
					&(),
					&investor,
				);

				portfolio
					.entry(TrancheCurrency::generate(pool_id, tranche_id))
					.and_modify(|p| {
						p.free_tranche_tokens = free_balance;
						p.reserved_tranche_tokens = reserved_balance;
					})
					.or_insert(
						InvestmentPortfolio::<Balance, CurrencyId>::new(pool_currency)
							.with_free_tranche_tokens(free_balance)
							.with_reserved_tranche_tokens(reserved_balance),
					);
			}
		}

		// Set pending invest currency and claimable tranche tokens
		for invest_id in pallet_investments::InvestOrders::<T>::iter_key_prefix(&investor) {
			let pool_currency = pallet_pool_system::Pallet::<T>::currency_for(invest_id.of_pool())
				.ok_or(DispatchError::Other("Pool must exist; qed"))?;

			// Collect such that we can determine claimable tranche tokens
			// NOTE: Does not modify storage since RtAPI is readonly
			let _ =
				pallet_investments::Pallet::<T>::collect_investment(investor.clone(), invest_id);
			let amount = pallet_investments::InvestOrders::<T>::get(&investor, invest_id)
				.map(|order| order.amount())
				.unwrap_or_default();
			let free_tranche_tokens_new = pallet_restricted_tokens::Pallet::<T>::reducible_balance(
				invest_id.into(),
				&investor,
				Preservation::Preserve,
				Fortitude::Polite,
			);

			portfolio
				.entry(invest_id)
				.and_modify(|p| {
					p.pending_invest_currency = amount;
					if p.free_tranche_tokens < free_tranche_tokens_new {
						p.claimable_tranche_tokens =
							free_tranche_tokens_new.saturating_sub(p.free_tranche_tokens);
					}
				})
				.or_insert(
					InvestmentPortfolio::<Balance, CurrencyId>::new(pool_currency)
						.with_pending_invest_currency(amount)
						.with_claimable_tranche_tokens(free_tranche_tokens_new),
				);
		}

		// Set pending tranche tokens and claimable invest currency
		for invest_id in pallet_investments::RedeemOrders::<T>::iter_key_prefix(&investor) {
			let pool_currency = pallet_pool_system::Pallet::<T>::currency_for(invest_id.of_pool())
				.ok_or(DispatchError::Other("Pool must exist; qed"))?;

			let balance_before = pallet_restricted_tokens::Pallet::<T>::reducible_balance(
				pool_currency,
				&investor,
				Preservation::Preserve,
				Fortitude::Polite,
			);

			// Collect such that we can determine claimable invest currency
			// NOTE: Does not modify storage since RtAPI is readonly
			let _ =
				pallet_investments::Pallet::<T>::collect_redemption(investor.clone(), invest_id);
			let amount = pallet_investments::RedeemOrders::<T>::get(&investor, invest_id)
				.map(|order| order.amount())
				.unwrap_or_default();
			let balance_after = pallet_restricted_tokens::Pallet::<T>::reducible_balance(
				pool_currency,
				&investor,
				Preservation::Preserve,
				Fortitude::Polite,
			);

			portfolio
				.entry(invest_id)
				.and_modify(|p| {
					p.pending_redeem_tranche_tokens = amount;
					if balance_before < balance_after {
						p.claimable_currency = balance_after.saturating_sub(balance_before);
					}
				})
				.or_insert(
					InvestmentPortfolio::<Balance, CurrencyId>::new(pool_currency)
						.with_pending_redeem_tranche_tokens(amount)
						.with_claimable_currency(balance_after),
				);
		}

		Ok(portfolio.into_iter().collect())
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
		fn destination(self) -> staging_xcm::latest::Location {
			Default::default()
		}
	}
}

pub mod foreign_investments {
	use cfg_primitives::{conversion::convert_balance_decimals, Balance};
	use cfg_traits::IdentityCurrencyConversion;
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
			use crate::origins::gov::types::HalfOfCouncil;

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

pub mod rewards {
	frame_support::parameter_types! {
		#[derive(scale_info::TypeInfo)]
		pub const SingleCurrencyMovement: u32 = 1;
	}
}

/// Helpers to deal with configuring the message queue in the runtime.
pub mod message_queue {
	use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
	use frame_support::traits::{QueueFootprint, QueuePausedQuery};
	use pallet_message_queue::OnQueueChanged;
	use sp_std::marker::PhantomData;

	pub struct NarrowOriginToSibling<Inner>(PhantomData<Inner>);
	impl<Inner: QueuePausedQuery<ParaId>> QueuePausedQuery<AggregateMessageOrigin>
		for NarrowOriginToSibling<Inner>
	{
		fn is_paused(origin: &AggregateMessageOrigin) -> bool {
			match origin {
				AggregateMessageOrigin::Sibling(id) => Inner::is_paused(id),
				_ => false,
			}
		}
	}

	impl<Inner: OnQueueChanged<ParaId>> OnQueueChanged<AggregateMessageOrigin>
		for NarrowOriginToSibling<Inner>
	{
		fn on_queue_changed(origin: AggregateMessageOrigin, fp: QueueFootprint) {
			if let AggregateMessageOrigin::Sibling(id) = origin {
				Inner::on_queue_changed(id, fp)
			}
		}
	}

	pub struct ParaIdToSibling;
	impl sp_runtime::traits::Convert<ParaId, AggregateMessageOrigin> for ParaIdToSibling {
		fn convert(para_id: ParaId) -> AggregateMessageOrigin {
			AggregateMessageOrigin::Sibling(para_id)
		}
	}
}
