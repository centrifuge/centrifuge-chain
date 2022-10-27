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
use cfg_types::{
	CurrencyAdminRoles, CurrencyId, InvestmentInfo, PermissionRoles, PermissionedCurrencyRole,
	PoolAdminRoles, PoolRole, Role,
};
use frame_support::{
	sp_runtime::traits::Saturating,
	traits::{EnsureOrigin, EnsureOriginWithArg, UnixTime},
};
use frame_system::RawOrigin;
use sp_std::marker::PhantomData;

use super::*;

type AccountId = u64;

/// This OrmlAssetRegistry::AuthorityOrigin implementation is used for our pallet-loans
/// and pallet-pools Mocks. We overwrite this because of the `type AccountId = u64`.
/// In the runtime tests, we use proper AccountIds, in the Mocks, we use 1,2,3,... .
/// Therefore, we implement `AuthorityOrigin` and use the `u64` type for the AccountId.
///
/// Use this implementation only when setting up Mocks with simple AccountIds.
pub struct AuthorityOrigin<
	// The origin type
	Origin,
	// The default EnsureOrigin impl used to authorize all
	// assets besides tranche tokens.
	DefaultEnsureOrigin,
>(PhantomData<(Origin, DefaultEnsureOrigin)>);

impl<
		Origin: Into<Result<RawOrigin<AccountId>, Origin>> + From<RawOrigin<AccountId>>,
		EnsureRoot: EnsureOrigin<Origin>,
	> EnsureOriginWithArg<Origin, Option<CurrencyId>> for AuthorityOrigin<Origin, EnsureRoot>
{
	type Success = ();

	fn try_origin(origin: Origin, asset_id: &Option<CurrencyId>) -> Result<Self::Success, Origin> {
		match asset_id {
			// Only the pools pallet should directly register/update tranche tokens
			Some(CurrencyId::Tranche(_, _)) => Err(origin),

			// Any other `asset_id` defaults to EnsureRoot
			_ => EnsureRoot::try_origin(origin).map(|_| ()),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin(_asset_id: &Option<CurrencyId>) -> Origin {
		todo!()
	}
}

impl<AccountId, Currency, InvestmentId> InvestmentProperties<AccountId>
	for InvestmentInfo<AccountId, Currency, InvestmentId>
where
	AccountId: Clone,
	Currency: Clone,
	InvestmentId: Clone,
{
	type Currency = Currency;
	type Id = InvestmentId;

	fn owner(&self) -> AccountId {
		self.owner.clone()
	}

	fn id(&self) -> Self::Id {
		self.id.clone()
	}

	fn payment_currency(&self) -> Self::Currency {
		self.payment_currency.clone()
	}
}

impl<Now, MinDelay, TrancheId, Moment> Properties
	for PermissionRoles<Now, MinDelay, TrancheId, Moment>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord + Copy,
	TrancheId: PartialEq + PartialOrd,
{
	type Error = ();
	type Ok = ();
	type Property = Role<TrancheId, Moment>;

	fn exists(&self, property: Self::Property) -> bool {
		match property {
			Role::PoolRole(pool_role) => match pool_role {
				PoolRole::Borrower => self.pool_admin.contains(PoolAdminRoles::BORROWER),
				PoolRole::LiquidityAdmin => {
					self.pool_admin.contains(PoolAdminRoles::LIQUIDITY_ADMIN)
				}
				PoolRole::PoolAdmin => self.pool_admin.contains(PoolAdminRoles::POOL_ADMIN),
				PoolRole::PricingAdmin => self.pool_admin.contains(PoolAdminRoles::PRICING_ADMIN),
				PoolRole::MemberListAdmin => {
					self.pool_admin.contains(PoolAdminRoles::MEMBER_LIST_ADMIN)
				}
				PoolRole::LoanAdmin => self.pool_admin.contains(PoolAdminRoles::RISK_ADMIN),
				PoolRole::TrancheInvestor(id, _) => self.tranche_investor.contains(id),
			},
			Role::PermissionedCurrencyRole(permissioned_currency_role) => {
				match permissioned_currency_role {
					PermissionedCurrencyRole::Holder(_) => {
						self.permissioned_asset_holder.contains()
					}
					PermissionedCurrencyRole::Manager => self
						.currency_admin
						.contains(CurrencyAdminRoles::PERMISSIONED_ASSET_MANAGER),
					PermissionedCurrencyRole::Issuer => self
						.currency_admin
						.contains(CurrencyAdminRoles::PERMISSIONED_ASSET_ISSUER),
				}
			}
		}
	}

	fn empty(&self) -> bool {
		self.pool_admin.is_empty()
			&& self.currency_admin.is_empty()
			&& self.tranche_investor.is_empty()
			&& self.permissioned_asset_holder.is_empty()
	}

	fn rm(&mut self, property: Self::Property) -> Result<(), ()> {
		match property {
			Role::PoolRole(pool_role) => match pool_role {
				PoolRole::Borrower => Ok(self.pool_admin.remove(PoolAdminRoles::BORROWER)),
				PoolRole::LiquidityAdmin => {
					Ok(self.pool_admin.remove(PoolAdminRoles::LIQUIDITY_ADMIN))
				}
				PoolRole::PoolAdmin => Ok(self.pool_admin.remove(PoolAdminRoles::POOL_ADMIN)),
				PoolRole::PricingAdmin => Ok(self.pool_admin.remove(PoolAdminRoles::PRICING_ADMIN)),
				PoolRole::MemberListAdmin => {
					Ok(self.pool_admin.remove(PoolAdminRoles::MEMBER_LIST_ADMIN))
				}
				PoolRole::LoanAdmin => Ok(self.pool_admin.remove(PoolAdminRoles::RISK_ADMIN)),
				PoolRole::TrancheInvestor(id, delta) => self.tranche_investor.remove(id, delta),
			},
			Role::PermissionedCurrencyRole(permissioned_currency_role) => {
				match permissioned_currency_role {
					PermissionedCurrencyRole::Holder(delta) => {
						self.permissioned_asset_holder.remove(delta)
					}
					PermissionedCurrencyRole::Manager => Ok(self
						.currency_admin
						.remove(CurrencyAdminRoles::PERMISSIONED_ASSET_MANAGER)),
					PermissionedCurrencyRole::Issuer => Ok(self
						.currency_admin
						.remove(CurrencyAdminRoles::PERMISSIONED_ASSET_ISSUER)),
				}
			}
		}
	}

	fn add(&mut self, property: Self::Property) -> Result<(), ()> {
		match property {
			Role::PoolRole(pool_role) => match pool_role {
				PoolRole::Borrower => Ok(self.pool_admin.insert(PoolAdminRoles::BORROWER)),
				PoolRole::LiquidityAdmin => {
					Ok(self.pool_admin.insert(PoolAdminRoles::LIQUIDITY_ADMIN))
				}
				PoolRole::PoolAdmin => Ok(self.pool_admin.insert(PoolAdminRoles::POOL_ADMIN)),
				PoolRole::PricingAdmin => Ok(self.pool_admin.insert(PoolAdminRoles::PRICING_ADMIN)),
				PoolRole::MemberListAdmin => {
					Ok(self.pool_admin.insert(PoolAdminRoles::MEMBER_LIST_ADMIN))
				}
				PoolRole::LoanAdmin => Ok(self.pool_admin.insert(PoolAdminRoles::RISK_ADMIN)),
				PoolRole::TrancheInvestor(id, delta) => self.tranche_investor.insert(id, delta),
			},
			Role::PermissionedCurrencyRole(permissioned_currency_role) => {
				match permissioned_currency_role {
					PermissionedCurrencyRole::Holder(delta) => {
						self.permissioned_asset_holder.insert(delta)
					}
					PermissionedCurrencyRole::Manager => Ok(self
						.currency_admin
						.insert(CurrencyAdminRoles::PERMISSIONED_ASSET_MANAGER)),
					PermissionedCurrencyRole::Issuer => Ok(self
						.currency_admin
						.insert(CurrencyAdminRoles::PERMISSIONED_ASSET_ISSUER)),
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use core::time::Duration;

	use frame_support::parameter_types;

	///! Tests for some types in the common section for our runtimes
	use super::*;

	parameter_types! {
		pub const MinDelay: u64 = 4;
	}

	struct Now(core::time::Duration);

	impl Now {
		fn pass(delta: u64) {
			unsafe {
				let current = NOW_HOLDER;
				NOW_HOLDER = current + delta;
			};
		}

		fn set(now: u64) {
			unsafe {
				NOW_HOLDER = now;
			};
		}
	}

	static mut NOW_HOLDER: u64 = 0;

	impl UnixTime for Now {
		fn now() -> Duration {
			unsafe { Duration::new(NOW_HOLDER, 0) }
		}
	}

	/// The exists call does not care what is passed as moment. This type shall reflect that
	const UNION: u64 = 0u64;

	/// The tranceh id type we use in our runtime-common. But we don't want a dependency here.
	type TrancheId = [u8; 16];

	fn into_tranche_id(val: u8) -> TrancheId {
		[val; 16]
	}

	#[test]
	fn permission_roles_work() {
		assert!(PermissionRoles::<Now, MinDelay, TrancheId>::default().empty());

		let mut roles = PermissionRoles::<Now, MinDelay, TrancheId>::default();

		// Updating works only when increasing permissions
		assert!(roles
			.add(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(30),
				10
			)))
			.is_ok());
		assert!(roles
			.add(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(30),
				9
			)))
			.is_err());
		assert!(roles
			.add(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(30),
				11
			)))
			.is_ok());

		// Test zero-tranche handling
		assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(0),
			UNION
		))));
		assert!(roles
			.add(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(0),
				MinDelay::get()
			)))
			.is_ok());
		assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(0),
			UNION
		))));

		// Removing before MinDelay fails
		assert!(roles
			.rm(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(0),
				0
			)))
			.is_err());
		Now::pass(1);
		assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(0),
			UNION
		))));
		assert!(roles
			.rm(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(0),
				MinDelay::get() - 1
			)))
			.is_err());
		assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(0),
			UNION
		))));
		Now::set(0);

		// Removing after MinDelay works (i.e. this is after min_delay the account will be invalid)
		assert!(roles
			.rm(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(0),
				MinDelay::get()
			)))
			.is_ok());
		Now::pass(6);
		assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(0),
			UNION
		))));
		Now::set(0);

		// Multiple tranches work
		assert!(roles
			.add(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(1),
				MinDelay::get()
			)))
			.is_ok());
		assert!(roles
			.add(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(2),
				MinDelay::get()
			)))
			.is_ok());
		assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(1),
			UNION
		))));
		assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(2),
			UNION
		))));

		// Adding roles works normally
		assert!(roles.add(Role::PoolRole(PoolRole::LiquidityAdmin)).is_ok());
		assert!(roles.add(Role::PoolRole(PoolRole::MemberListAdmin)).is_ok());
		assert!(roles.exists(Role::PoolRole(PoolRole::LiquidityAdmin)));
		assert!(roles.exists(Role::PoolRole(PoolRole::MemberListAdmin)));

		// Role exists for as long as permission is given
		assert!(roles
			.add(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(8),
				MinDelay::get() + 2
			)))
			.is_ok());
		assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(8),
			UNION
		))));
		Now::pass(MinDelay::get() + 2);
		assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(8),
			UNION
		))));
		Now::pass(1);
		assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(8),
			UNION
		))));
		Now::set(0);

		// Role must be added for at least min_delay
		assert!(roles
			.add(Role::PoolRole(PoolRole::TrancheInvestor(
				into_tranche_id(5),
				MinDelay::get() - 1
			)))
			.is_err());
		assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(5),
			UNION
		))));

		// Removing roles work normally for Non-TrancheInvestor roles
		assert!(roles.rm(Role::PoolRole(PoolRole::LiquidityAdmin)).is_ok());
		assert!(roles.rm(Role::PoolRole(PoolRole::MemberListAdmin)).is_ok());
		assert!(!roles.exists(Role::PoolRole(PoolRole::LiquidityAdmin)));
		assert!(!roles.exists(Role::PoolRole(PoolRole::MemberListAdmin)));
	}
}
