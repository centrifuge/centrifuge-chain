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

use cfg_traits::{Properties, Seconds, TimeAsSecs};
use frame_support::{traits::Get, BoundedVec};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::{
	cmp::{Ord, PartialEq, PartialOrd},
	marker::PhantomData,
};

/// PoolRole can hold any type of role specific functions a user can do on a
/// given pool.
// NOTE: In order to not carry around the TrancheId type all the time, we give it a
// default.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum PoolRole<TrancheId = [u8; 16]> {
	PoolAdmin,
	Borrower,
	PricingAdmin,
	LiquidityAdmin,
	InvestorAdmin,
	LoanAdmin,
	TrancheInvestor(TrancheId, Seconds),
	PODReadAccess,
	FrozenTrancheInvestor(TrancheId),
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum PermissionedCurrencyRole {
	/// This role can hold & transfer tokens
	Holder(Seconds),
	/// This role can add/remove holders
	Manager,
	/// This role can mint/burn tokens
	Issuer,
}

/// The Role enum is used by the permissions pallet,
/// to specify which role an account has within a
/// specific scope.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Role<TrancheId = [u8; 16]> {
	/// Roles that apply to a specific pool.
	PoolRole(PoolRole<TrancheId>),
	/// Roles that apply to a specific permissioned currency.
	PermissionedCurrencyRole(PermissionedCurrencyRole),
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum PermissionScope<PoolId, CurrencyId> {
	Pool(PoolId),
	Currency(CurrencyId),
}

/// This is only used by the permission pallet benchmarks.
// TODO: use conditional compilation to only add this on benchmarks and tests.
// #[cfg(any(test, feature = "runtime-benchmarks"))]
impl<PoolId, CurrencyId> Default for PermissionScope<PoolId, CurrencyId>
where
	PoolId: Default,
{
	fn default() -> Self {
		Self::Pool(PoolId::default())
	}
}

bitflags::bitflags! {
	/// The current admin roles we support
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct PoolAdminRoles: u32 {
		const POOL_ADMIN = 0b00000001;
		const BORROWER  = 0b00000010;
		const PRICING_ADMIN = 0b00000100;
		const LIQUIDITY_ADMIN = 0b00001000;
		const INVESTOR_ADMIN = 0b00010000;
		const RISK_ADMIN = 0b00100000;
		const POD_READ_ACCESS = 0b01000000;
	}

	/// The current admin roles we support
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct CurrencyAdminRoles: u32 {
		const PERMISSIONED_ASSET_MANAGER = 0b00000001;
		const PERMISSIONED_ASSET_ISSUER  = 0b00000010;
	}
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
pub struct PermissionedCurrencyHolderInfo {
	permissioned_till: Seconds,
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
pub struct TrancheInvestorInfo<TrancheId> {
	tranche_id: TrancheId,
	permissioned_till: Seconds,
	is_frozen: bool,
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
pub struct PermissionedCurrencyHolders<Now, MinDelay> {
	info: Option<PermissionedCurrencyHolderInfo>,
	_phantom: PhantomData<(Now, MinDelay)>,
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
pub struct TrancheInvestors<Now, MinDelay, TrancheId, MaxTranches: Get<u32>> {
	info: BoundedVec<TrancheInvestorInfo<TrancheId>, MaxTranches>,
	_phantom: PhantomData<(Now, MinDelay)>,
}

/// The structure that we store in the pallet-permissions storage
/// This here implements trait Properties.
#[derive(Encode, Decode, TypeInfo, Clone, Eq, PartialEq, Debug, MaxEncodedLen)]
pub struct PermissionRoles<Now, MinDelay, TrancheId, MaxTranches: Get<u32>> {
	pool_admin: PoolAdminRoles,
	currency_admin: CurrencyAdminRoles,
	permissioned_asset_holder: PermissionedCurrencyHolders<Now, MinDelay>,
	tranche_investor: TrancheInvestors<Now, MinDelay, TrancheId, MaxTranches>,
}

impl<Now, MinDelay> Default for PermissionedCurrencyHolders<Now, MinDelay>
where
	Now: TimeAsSecs,
	MinDelay: Get<Seconds>,
{
	fn default() -> Self {
		Self {
			info: None,
			_phantom: Default::default(),
		}
	}
}

impl<Now, MinDelay, TrancheId, MaxTranches> Default
	for TrancheInvestors<Now, MinDelay, TrancheId, MaxTranches>
where
	Now: TimeAsSecs,
	MinDelay: Get<Seconds>,
	TrancheId: PartialEq + PartialOrd,
	MaxTranches: Get<u32>,
{
	fn default() -> Self {
		Self {
			info: BoundedVec::default(),
			_phantom: Default::default(),
		}
	}
}

impl<Now, MinDelay, TrancheId, MaxTranches> Default
	for PermissionRoles<Now, MinDelay, TrancheId, MaxTranches>
where
	Now: TimeAsSecs,
	MinDelay: Get<Seconds>,
	TrancheId: PartialEq + PartialOrd,
	MaxTranches: Get<u32>,
{
	fn default() -> Self {
		Self {
			pool_admin: PoolAdminRoles::empty(),
			currency_admin: CurrencyAdminRoles::empty(),
			permissioned_asset_holder: PermissionedCurrencyHolders::<Now, MinDelay>::default(),
			tranche_investor: TrancheInvestors::<Now, MinDelay, TrancheId, MaxTranches>::default(),
		}
	}
}

impl<Now, MinDelay, TrancheId, MaxTranches> Properties
	for PermissionRoles<Now, MinDelay, TrancheId, MaxTranches>
where
	Now: TimeAsSecs,
	MinDelay: Get<Seconds>,
	TrancheId: PartialEq + PartialOrd + Copy,
	MaxTranches: Get<u32>,
{
	type Error = ();
	type Ok = ();
	type Property = Role<TrancheId>;

	fn exists(&self, property: Self::Property) -> bool {
		match property {
			Role::PoolRole(pool_role) => match pool_role {
				PoolRole::Borrower => self.pool_admin.contains(PoolAdminRoles::BORROWER),
				PoolRole::LiquidityAdmin => {
					self.pool_admin.contains(PoolAdminRoles::LIQUIDITY_ADMIN)
				}
				PoolRole::PoolAdmin => self.pool_admin.contains(PoolAdminRoles::POOL_ADMIN),
				PoolRole::PricingAdmin => self.pool_admin.contains(PoolAdminRoles::PRICING_ADMIN),
				PoolRole::InvestorAdmin => self.pool_admin.contains(PoolAdminRoles::INVESTOR_ADMIN),
				PoolRole::LoanAdmin => self.pool_admin.contains(PoolAdminRoles::RISK_ADMIN),
				PoolRole::TrancheInvestor(id, validity) => {
					self.tranche_investor.contains(id, validity)
				}
				PoolRole::PODReadAccess => {
					self.pool_admin.contains(PoolAdminRoles::POD_READ_ACCESS)
				}
				PoolRole::FrozenTrancheInvestor(id) => self.tranche_investor.contains_frozen(id),
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

	fn get(&self, property: Self::Property) -> Option<Self::Property> {
		match property {
			Role::PoolRole(PoolRole::TrancheInvestor(id, _validity)) => {
				self.tranche_investor.get(id).map(|info| {
					Role::PoolRole(PoolRole::TrancheInvestor(
						info.tranche_id,
						info.permissioned_till,
					))
				})
			}
			Role::PermissionedCurrencyRole(PermissionedCurrencyRole::Holder(_validity)) => {
				self.permissioned_asset_holder.get().map(|info| {
					Role::PermissionedCurrencyRole(PermissionedCurrencyRole::Holder(
						info.permissioned_till,
					))
				})
			}
			role => Self::exists(&self, role).then(|| role),
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
				PoolRole::InvestorAdmin => {
					Ok(self.pool_admin.remove(PoolAdminRoles::INVESTOR_ADMIN))
				}
				PoolRole::LoanAdmin => Ok(self.pool_admin.remove(PoolAdminRoles::RISK_ADMIN)),
				PoolRole::TrancheInvestor(id, delta) => self.tranche_investor.remove(id, delta),
				PoolRole::PODReadAccess => {
					Ok(self.pool_admin.remove(PoolAdminRoles::POD_READ_ACCESS))
				}
				PoolRole::FrozenTrancheInvestor(id) => self.tranche_investor.unfreeze(id),
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
				PoolRole::InvestorAdmin => {
					Ok(self.pool_admin.insert(PoolAdminRoles::INVESTOR_ADMIN))
				}
				PoolRole::LoanAdmin => Ok(self.pool_admin.insert(PoolAdminRoles::RISK_ADMIN)),
				PoolRole::TrancheInvestor(id, delta) => self.tranche_investor.insert(id, delta),
				PoolRole::PODReadAccess => {
					Ok(self.pool_admin.insert(PoolAdminRoles::POD_READ_ACCESS))
				}
				PoolRole::FrozenTrancheInvestor(id) => self.tranche_investor.freeze(id),
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

impl<Now, MinDelay> PermissionedCurrencyHolders<Now, MinDelay>
where
	Now: TimeAsSecs,
	MinDelay: Get<Seconds>,
{
	pub fn empty() -> Self {
		Self::default()
	}

	pub fn is_empty(&self) -> bool {
		self.info.is_none()
	}

	fn validity(&self, delta: Seconds) -> Result<Seconds, ()> {
		if delta < MinDelay::get() {
			Err(())
		} else {
			let now = <Now as TimeAsSecs>::now();
			let req_validity = now.saturating_add(delta);
			Ok(req_validity)
		}
	}

	pub fn contains(&self) -> bool {
		if let Some(info) = &self.info {
			info.permissioned_till >= <Now as TimeAsSecs>::now()
		} else {
			false
		}
	}

	pub fn get(&self) -> Option<&PermissionedCurrencyHolderInfo> {
		self.info.iter().find(|_| Self::contains(&self))
	}

	#[allow(clippy::result_unit_err)]
	pub fn remove(&mut self, delta: Seconds) -> Result<(), ()> {
		if let Some(info) = &self.info {
			let valid_till = &info.permissioned_till;
			let now = <Now as TimeAsSecs>::now();

			if *valid_till < now {
				// The account is already invalid. Hence no more grace period
				Err(())
			} else {
				// Ensure that permissioned_till is at least now + min_delay.
				let permissioned_till = self.validity(delta)?;
				self.info = Some(PermissionedCurrencyHolderInfo { permissioned_till });
				Ok(())
			}
		} else {
			Err(())
		}
	}

	#[allow(clippy::result_unit_err)]
	pub fn insert(&mut self, delta: Seconds) -> Result<(), ()> {
		let validity = self.validity(delta)?;

		match &self.info {
			Some(info) if info.permissioned_till > validity => Err(()),
			_ => {
				self.info = Some(PermissionedCurrencyHolderInfo {
					permissioned_till: validity,
				});

				Ok(())
			}
		}
	}
}

impl<Now, MinDelay, TrancheId, MaxTranches> TrancheInvestors<Now, MinDelay, TrancheId, MaxTranches>
where
	Now: TimeAsSecs,
	MinDelay: Get<Seconds>,
	TrancheId: PartialEq + PartialOrd,
	MaxTranches: Get<u32>,
{
	pub fn empty() -> Self {
		Self::default()
	}

	pub fn is_empty(&self) -> bool {
		self.info.is_empty()
	}

	fn validity(&self, delta: Seconds) -> Result<Seconds, ()> {
		if delta < MinDelay::get() {
			Err(())
		} else {
			let now = <Now as TimeAsSecs>::now();
			let req_validity = now.saturating_add(delta);
			Ok(req_validity)
		}
	}

	pub fn contains(&self, tranche: TrancheId, validity: Seconds) -> bool {
		self.info.iter().any(|info| {
			info.tranche_id == tranche
				&& info.permissioned_till == validity
				&& validity >= <Now as TimeAsSecs>::now()
		})
	}

	pub fn get(&self, tranche: TrancheId) -> Option<&TrancheInvestorInfo<TrancheId>> {
		self.info.iter().find(|info| {
			info.tranche_id == tranche && info.permissioned_till >= <Now as TimeAsSecs>::now()
		})
	}

	pub fn contains_frozen(&self, tranche: TrancheId) -> bool {
		self.info
			.iter()
			.any(|info| info.tranche_id == tranche && info.is_frozen)
	}

	#[allow(clippy::result_unit_err)]
	pub fn remove(&mut self, tranche: TrancheId, delta: Seconds) -> Result<(), ()> {
		if let Some(index) = self.info.iter().position(|info| info.tranche_id == tranche) {
			let valid_till = &self.info[index].permissioned_till;
			let now = <Now as TimeAsSecs>::now();

			if *valid_till < now {
				// The account is already invalid. Hence no more grace period
				Err(())
			} else {
				// Ensure that permissioned_till is at least now + min_delay.
				Ok(self.info[index].permissioned_till = self.validity(delta)?)
			}
		} else {
			Err(())
		}
	}

	#[allow(clippy::result_unit_err)]
	pub fn insert(&mut self, tranche: TrancheId, delta: Seconds) -> Result<(), ()> {
		let validity = self.validity(delta)?;

		if let Some(index) = self.info.iter().position(|info| info.tranche_id == tranche) {
			if self.info[index].permissioned_till > validity {
				Err(())
			} else {
				Ok(self.info[index].permissioned_till = validity)
			}
		} else {
			self.info
				.try_push(TrancheInvestorInfo {
					tranche_id: tranche,
					permissioned_till: validity,
					is_frozen: false,
				})
				.map_err(|_| ())
		}
	}

	#[allow(clippy::result_unit_err)]
	pub fn freeze(&mut self, tranche: TrancheId) -> Result<(), ()> {
		if let Some(investor) = self.info.iter_mut().find(|t| t.tranche_id == tranche) {
			investor.is_frozen = true;
		}
		Ok(())
	}

	#[allow(clippy::result_unit_err)]
	pub fn unfreeze(&mut self, tranche: TrancheId) -> Result<(), ()> {
		if let Some(investor) = self.info.iter_mut().find(|t| t.tranche_id == tranche) {
			investor.is_frozen = false;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use core::time::Duration;
	use std::cell::RefCell;

	use frame_support::{assert_ok, parameter_types, traits::UnixTime};

	use super::*;

	parameter_types! {
		pub const MinDelay: u64 = MIN_DELAY;
		pub const MaxTranches: u32 = 5;
	}

	// Thread-local storage for `NOW_HOLDER`
	// Each thread will have its own independent value of `NOW_HOLDER`
	thread_local! {
		static NOW_HOLDER: RefCell<u64> = RefCell::new(0);
	}

	// Struct representing the Unix time logic
	struct Now;

	impl Now {
		fn pass(delta: u64) {
			NOW_HOLDER.with(|now_holder| {
				*now_holder.borrow_mut() += delta;
			});
		}

		#[allow(dead_code)]

		fn set(t: u64) {
			NOW_HOLDER.with(|now_holder| {
				*now_holder.borrow_mut() = t;
			});
		}
	}

	// Implementing the `UnixTime` trait for `Now`
	impl UnixTime for Now {
		fn now() -> Duration {
			NOW_HOLDER.with(|now_holder| Duration::new(*now_holder.borrow(), 0))
		}
	}

	/// The default validity
	const VALIDITY: u64 = 14;
	/// The minimum delay
	const MIN_DELAY: u64 = 10;
	/// The tranche id type we use in our runtime-common. But we don't want a
	/// dependency here.
	type TrancheId = [u8; 16];

	fn into_tranche_id(val: u8) -> TrancheId {
		[val; 16]
	}

	#[test]
	fn default_roles() {
		assert!(PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default().empty());
	}

	#[test]
	fn default_time() {
		assert!(<Now as UnixTime>::now().is_zero());
	}

	#[test]
	fn time_manipulation() {
		Now::set(10);
		assert_eq!(<Now as UnixTime>::now(), Duration::new(10, 0));

		Now::pass(100);
		assert_eq!(<Now as UnixTime>::now(), Duration::new(110, 0));

		Now::set(5);
		assert_eq!(<Now as UnixTime>::now(), Duration::new(5, 0));
	}

	mod pool_role {
		use super::*;

		mod non_tranche_investor {
			use super::*;
			#[test]
			fn success_with_adding_roles() {
				let mut roles = PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

				for role in [
					Role::PoolRole(PoolRole::Borrower),
					Role::PoolRole(PoolRole::LiquidityAdmin),
					Role::PoolRole(PoolRole::PoolAdmin),
					Role::PoolRole(PoolRole::PricingAdmin),
					Role::PoolRole(PoolRole::InvestorAdmin),
					Role::PoolRole(PoolRole::LoanAdmin),
					Role::PoolRole(PoolRole::PODReadAccess),
				] {
					assert_ok!(roles.add(role));
					assert!(roles.exists(role));
					assert_eq!(roles.get(role), Some(role));
				}
			}
			#[test]
			fn success_with_removing_roles() {
				let mut roles = PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

				for role in [
					Role::PoolRole(PoolRole::Borrower),
					Role::PoolRole(PoolRole::LiquidityAdmin),
					Role::PoolRole(PoolRole::PoolAdmin),
					Role::PoolRole(PoolRole::PricingAdmin),
					Role::PoolRole(PoolRole::InvestorAdmin),
					Role::PoolRole(PoolRole::LoanAdmin),
					Role::PoolRole(PoolRole::PODReadAccess),
				] {
					assert_ok!(roles.add(role));
					assert_ok!(roles.rm(role));
					assert!(!roles.exists(role));
					assert!(roles.get(role).is_none());
				}
			}
		}

		mod tranche_investor {
			use super::*;

			mod success {
				use super::*;

				#[test]
				fn with_updating_requires_increasing_validity() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY
					))));
					assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY
					))));
					assert_eq!(
						roles.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							Default::default()
						))),
						Some(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							VALIDITY
						)))
					);

					assert!(roles
						.add(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							VALIDITY - 1
						)))
						.is_err());

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY + 1
					))));
					assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY + 1
					))));
					assert_eq!(
						roles.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							Default::default()
						))),
						Some(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							VALIDITY + 1
						)))
					);
				}

				#[test]
				fn with_zero_tranche() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						VALIDITY
					))));

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						VALIDITY
					))));

					assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						VALIDITY
					))));
					assert_eq!(
						roles.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(0),
							Default::default()
						))),
						Some(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(0),
							VALIDITY
						)))
					);
				}

				#[test]
				/// TrancheInvestor is invalid in block after validity
				fn with_invalidation_after_validity_expiration() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						VALIDITY
					))));

					Now::pass(VALIDITY);
					assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						VALIDITY
					))));
					assert_eq!(
						roles.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(0),
							Default::default()
						))),
						Some(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(0),
							VALIDITY
						)))
					);

					Now::pass(1);
					assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						VALIDITY
					))));
					assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						1
					))));
					assert!(roles
						.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(0),
							1
						)))
						.is_none());
				}

				#[test]
				fn with_adding_multiple_tranches() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY
					))));
					assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY
					))));

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(2),
						VALIDITY
					))));
					assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(2),
						VALIDITY
					))));

					assert_eq!(
						roles.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							Default::default()
						))),
						Some(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							VALIDITY
						)))
					);
					assert_eq!(
						roles.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(2),
							Default::default()
						))),
						Some(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(2),
							VALIDITY
						)))
					);
				}

				#[test]
				fn with_reducing_validity() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY
					))));

					assert_ok!(roles.rm(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						MIN_DELAY
					))));

					for i in VALIDITY..MIN_DELAY {
						assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							i
						))));
					}
					assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						MIN_DELAY
					))));
					assert_eq!(
						roles.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							Default::default()
						))),
						Some(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							MIN_DELAY
						)))
					);
				}

				#[test]
				fn with_freezing() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();
					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY
					))));

					assert_ok!(roles.add(Role::PoolRole(PoolRole::FrozenTrancheInvestor(
						into_tranche_id(1)
					))));
					assert!(roles.exists(Role::PoolRole(PoolRole::FrozenTrancheInvestor(
						into_tranche_id(1)
					))));
					assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY
					))));
					assert_eq!(
						roles.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							Default::default()
						))),
						Some(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							VALIDITY
						)))
					);

					assert_ok!(roles.rm(Role::PoolRole(PoolRole::FrozenTrancheInvestor(
						into_tranche_id(1)
					))));
					assert!(
						!roles.exists(Role::PoolRole(PoolRole::FrozenTrancheInvestor(
							into_tranche_id(1)
						)))
					);
					assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(1),
						VALIDITY
					))));
					assert_eq!(
						roles.get(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							Default::default()
						))),
						Some(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(1),
							VALIDITY
						)))
					);
				}
			}

			mod failure {
				use super::*;

				#[test]
				fn with_adding_with_lower_validity() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						VALIDITY
					))));

					for i in 0..VALIDITY {
						assert!(roles
							.add(Role::PoolRole(PoolRole::TrancheInvestor(
								into_tranche_id(0),
								i
							)))
							.is_err());
					}
					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						VALIDITY
					))));
				}

				#[test]
				fn with_removing_below_min_delay() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						MIN_DELAY
					))));

					for i in 0..MIN_DELAY {
						assert!(roles
							.rm(Role::PoolRole(PoolRole::TrancheInvestor(
								into_tranche_id(0),
								i
							)))
							.is_err());
					}
				}

				#[test]
				fn with_removing_in_past() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PoolRole(PoolRole::TrancheInvestor(
						into_tranche_id(0),
						VALIDITY
					))));

					Now::pass(VALIDITY + 1);
					assert!(roles
						.rm(Role::PoolRole(PoolRole::TrancheInvestor(
							into_tranche_id(0),
							VALIDITY
						)))
						.is_err());
				}
			}
		}
	}

	mod permissioned_currency_role {
		use super::*;

		#[test]
		fn manager_success_with_adding_removing() {
			let mut roles = PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

			assert_ok!(roles.add(Role::PermissionedCurrencyRole(
				PermissionedCurrencyRole::Manager
			)));
			assert!(roles.exists(Role::PermissionedCurrencyRole(
				PermissionedCurrencyRole::Manager
			)));
			assert_eq!(
				roles.get(Role::PermissionedCurrencyRole(
					PermissionedCurrencyRole::Manager
				)),
				Some(Role::PermissionedCurrencyRole(
					PermissionedCurrencyRole::Manager
				))
			);

			assert_ok!(roles.rm(Role::PermissionedCurrencyRole(
				PermissionedCurrencyRole::Manager
			)));
			assert!(!roles.exists(Role::PermissionedCurrencyRole(
				PermissionedCurrencyRole::Manager
			)));
			assert!(roles
				.get(Role::PermissionedCurrencyRole(
					PermissionedCurrencyRole::Manager
				))
				.is_none());
		}

		#[test]
		fn issuer_success_with_adding_removing() {
			let mut roles = PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

			assert_ok!(roles.add(Role::PermissionedCurrencyRole(
				PermissionedCurrencyRole::Issuer
			)));
			assert!(roles.exists(Role::PermissionedCurrencyRole(
				PermissionedCurrencyRole::Issuer
			)));
			assert_eq!(
				roles.get(Role::PermissionedCurrencyRole(
					PermissionedCurrencyRole::Issuer
				)),
				Some(Role::PermissionedCurrencyRole(
					PermissionedCurrencyRole::Issuer
				))
			);

			assert_ok!(roles.rm(Role::PermissionedCurrencyRole(
				PermissionedCurrencyRole::Issuer
			)));
			assert!(!roles.exists(Role::PermissionedCurrencyRole(
				PermissionedCurrencyRole::Issuer
			)));
			assert!(roles
				.get(Role::PermissionedCurrencyRole(
					PermissionedCurrencyRole::Issuer
				))
				.is_none());
		}

		mod holder {
			use super::*;

			mod success {
				use super::*;

				#[test]
				fn with_updating_requires_increasing_validity() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY)
					)));
					assert!(roles.exists(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY)
					)));
					assert_eq!(
						roles.get(Role::PermissionedCurrencyRole(
							PermissionedCurrencyRole::Holder(Default::default())
						)),
						Some(Role::PermissionedCurrencyRole(
							PermissionedCurrencyRole::Holder(VALIDITY)
						))
					);

					assert!(roles
						.add(Role::PermissionedCurrencyRole(
							PermissionedCurrencyRole::Holder(VALIDITY - 1)
						))
						.is_err());

					assert_ok!(roles.add(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY + 1)
					)));
					assert!(roles.exists(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY + 1)
					)));
					assert_eq!(
						roles.get(Role::PermissionedCurrencyRole(
							PermissionedCurrencyRole::Holder(Default::default())
						)),
						Some(Role::PermissionedCurrencyRole(
							PermissionedCurrencyRole::Holder(VALIDITY + 1)
						))
					);
				}

				#[test]
				/// TrancheInvestor is invalid in block after validity
				fn with_invalidation_after_validity_expiration() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY)
					)));

					Now::pass(VALIDITY);
					assert!(roles.exists(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY)
					)));

					Now::pass(1);
					assert!(!roles.exists(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY)
					)));
					assert!(!roles.exists(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(1)
					)));
					assert!(roles
						.get(Role::PermissionedCurrencyRole(
							PermissionedCurrencyRole::Holder(Default::default())
						))
						.is_none());
				}

				#[test]
				fn with_reducing_validity() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY)
					)));

					assert_ok!(roles.rm(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(MIN_DELAY)
					)));

					for i in VALIDITY..MIN_DELAY {
						assert!(!roles.exists(Role::PermissionedCurrencyRole(
							PermissionedCurrencyRole::Holder(i)
						)));
					}
					assert!(roles.exists(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(MIN_DELAY)
					)));
				}
			}

			mod failure {
				use super::*;

				#[test]
				fn with_adding_with_lower_validity() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY)
					)));

					for i in 0..VALIDITY {
						assert!(roles
							.add(Role::PermissionedCurrencyRole(
								PermissionedCurrencyRole::Holder(i)
							))
							.is_err());
					}
					assert_ok!(roles.add(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY)
					)));
				}

				#[test]
				fn with_removing_below_min_delay() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(MIN_DELAY)
					)));

					for i in 0..MIN_DELAY {
						assert!(roles
							.rm(Role::PermissionedCurrencyRole(
								PermissionedCurrencyRole::Holder(i)
							))
							.is_err());
					}
				}

				#[test]
				fn with_removing_in_past() {
					let mut roles =
						PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

					assert_ok!(roles.add(Role::PermissionedCurrencyRole(
						PermissionedCurrencyRole::Holder(VALIDITY)
					)));

					Now::pass(VALIDITY + 1);
					assert!(roles
						.rm(Role::PermissionedCurrencyRole(
							PermissionedCurrencyRole::Holder(VALIDITY)
						))
						.is_err());
				}
			}
		}
	}
}

pub mod v0 {
	use super::*;

	#[derive(Encode, Decode, TypeInfo, Clone, Eq, PartialEq, Debug, MaxEncodedLen)]
	pub struct PermissionRoles<Now, MinDelay, TrancheId, MaxTranches: Get<u32>> {
		pool_admin: PoolAdminRoles,
		currency_admin: CurrencyAdminRoles,
		permissioned_asset_holder: PermissionedCurrencyHolders<Now, MinDelay>,
		tranche_investor: TrancheInvestors<Now, MinDelay, TrancheId, MaxTranches>,
	}

	#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
	pub struct TrancheInvestors<Now, MinDelay, TrancheId, MaxTranches: Get<u32>> {
		info: BoundedVec<TrancheInvestorInfo<TrancheId>, MaxTranches>,
		_phantom: PhantomData<(Now, MinDelay)>,
	}

	#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
	pub struct TrancheInvestorInfo<TrancheId> {
		tranche_id: TrancheId,
		permissioned_till: Seconds,
	}

	impl<Now, MinDelay, TrancheId, MaxTranches: Get<u32>>
		TrancheInvestors<Now, MinDelay, TrancheId, MaxTranches>
	{
		fn migrate(self) -> super::TrancheInvestors<Now, MinDelay, TrancheId, MaxTranches> {
			super::TrancheInvestors::<Now, MinDelay, TrancheId, MaxTranches> {
				info: BoundedVec::truncate_from(
					self.info
						.into_iter()
						.map(|info| super::TrancheInvestorInfo {
							tranche_id: info.tranche_id,
							permissioned_till: info.permissioned_till,
							is_frozen: false,
						})
						.collect(),
				),
				_phantom: self._phantom,
			}
		}
	}

	impl<Now, MinDelay, TrancheId: Clone, MaxTranches: Get<u32>>
		PermissionRoles<Now, MinDelay, TrancheId, MaxTranches>
	{
		pub fn migrate(self) -> super::PermissionRoles<Now, MinDelay, TrancheId, MaxTranches> {
			super::PermissionRoles {
				pool_admin: self.pool_admin,
				currency_admin: self.currency_admin,
				permissioned_asset_holder: self.permissioned_asset_holder,
				tranche_investor: self.tranche_investor.migrate(),
			}
		}
	}
}
