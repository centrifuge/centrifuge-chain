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

use cfg_traits::Properties;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	sp_runtime::traits::Saturating,
	traits::{Get, UnixTime},
	BoundedVec,
};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::{
	cmp::{Ord, PartialEq, PartialOrd},
	marker::PhantomData,
};

/// PoolRole can hold any type of role specific functions a user can do on a given pool.
// NOTE: In order to not carry around the TrancheId and Moment types all the time, we give it a default.
//       In case the Role we provide does not match what we expect. I.e. if we change the Moment
//       type in our actual runtimes, then the compiler complains about it anyways.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum PoolRole<TrancheId = [u8; 16], Moment = u64> {
	PoolAdmin,
	Borrower,
	PricingAdmin,
	LiquidityAdmin,
	MemberListAdmin,
	LoanAdmin,
	TrancheInvestor(TrancheId, Moment),
	PODReadAccess,
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum PermissionedCurrencyRole<Moment = u64> {
	/// This role can hold & transfer tokens
	Holder(Moment),
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
pub enum Role<TrancheId = [u8; 16], Moment = u64> {
	/// Roles that apply to a specific pool.
	PoolRole(PoolRole<TrancheId, Moment>),
	/// Roles that apply to a specific permissioned currency.
	PermissionedCurrencyRole(PermissionedCurrencyRole<Moment>),
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum PermissionScope<PoolId, CurrencyId> {
	Pool(PoolId),
	Currency(CurrencyId),
}

/// This is only used by the permission pallet benchmarks.
// TODO: use conditional compilation to only add this on benchmarks and tests.
// #[cfg(any(test, feature = "runtime-benchmarks", feature = "test-benchmarks"))]
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
	#[derive(codec::Encode, codec::Decode, TypeInfo, MaxEncodedLen)]
	pub struct PoolAdminRoles: u32 {
		const POOL_ADMIN = 0b00000001;
		const BORROWER  = 0b00000010;
		const PRICING_ADMIN = 0b00000100;
		const LIQUIDITY_ADMIN = 0b00001000;
		const MEMBER_LIST_ADMIN = 0b00010000;
		const RISK_ADMIN = 0b00100000;
		const POD_READ_ACCESS = 0b01000000;
	}

	/// The current admin roles we support
	#[derive(codec::Encode, codec::Decode, TypeInfo, MaxEncodedLen)]
	pub struct CurrencyAdminRoles: u32 {
		const PERMISSIONED_ASSET_MANAGER = 0b00000001;
		const PERMISSIONED_ASSET_ISSUER  = 0b00000010;
	}
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
pub struct PermissionedCurrencyHolderInfo<Moment> {
	permissioned_till: Moment,
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
pub struct TrancheInvestorInfo<TrancheId, Moment> {
	tranche_id: TrancheId,
	permissioned_till: Moment,
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
pub struct PermissionedCurrencyHolders<Now, MinDelay, Moment> {
	info: Option<PermissionedCurrencyHolderInfo<Moment>>,
	_phantom: PhantomData<(Now, MinDelay)>,
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen)]
pub struct TrancheInvestors<Now, MinDelay, TrancheId, Moment, MaxTranches: Get<u32>> {
	info: BoundedVec<TrancheInvestorInfo<TrancheId, Moment>, MaxTranches>,
	_phantom: PhantomData<(Now, MinDelay)>,
}

/// The structure that we store in the pallet-permissions storage
/// This here implements trait Properties.
#[derive(Encode, Decode, TypeInfo, Clone, Eq, PartialEq, Debug, MaxEncodedLen)]
pub struct PermissionRoles<Now, MinDelay, TrancheId, MaxTranches: Get<u32>, Moment = u64> {
	pool_admin: PoolAdminRoles,
	currency_admin: CurrencyAdminRoles,
	permissioned_asset_holder: PermissionedCurrencyHolders<Now, MinDelay, Moment>,
	tranche_investor: TrancheInvestors<Now, MinDelay, TrancheId, Moment, MaxTranches>,
}

impl<Now, MinDelay, Moment> Default for PermissionedCurrencyHolders<Now, MinDelay, Moment>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord,
{
	fn default() -> Self {
		Self {
			info: None,
			_phantom: Default::default(),
		}
	}
}

impl<Now, MinDelay, TrancheId, Moment, MaxTranches> Default
	for TrancheInvestors<Now, MinDelay, TrancheId, Moment, MaxTranches>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord,
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

impl<Now, MinDelay, TrancheId, MaxTranches, Moment> Default
	for PermissionRoles<Now, MinDelay, TrancheId, MaxTranches, Moment>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord,
	TrancheId: PartialEq + PartialOrd,
	MaxTranches: Get<u32>,
{
	fn default() -> Self {
		Self {
			pool_admin: PoolAdminRoles::empty(),
			currency_admin: CurrencyAdminRoles::empty(),
			permissioned_asset_holder:
				PermissionedCurrencyHolders::<Now, MinDelay, Moment>::default(),
			tranche_investor:
				TrancheInvestors::<Now, MinDelay, TrancheId, Moment, MaxTranches>::default(),
		}
	}
}

/// The implementation of trait Properties for our PermissionsRoles does not care which Moment
/// is passed to the PoolRole::TrancheInvestor(TrancheId, Moment) variant.
/// This UNION shall reflect that and explain to the reader why it is passed here.
pub const UNION: u64 = 0;

impl<Now, MinDelay, TrancheId, MaxTranches, Moment> Properties
	for PermissionRoles<Now, MinDelay, TrancheId, MaxTranches, Moment>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord + Copy,
	TrancheId: PartialEq + PartialOrd,
	MaxTranches: Get<u32>,
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
				PoolRole::PODReadAccess => {
					self.pool_admin.contains(PoolAdminRoles::POD_READ_ACCESS)
				}
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
				PoolRole::PODReadAccess => {
					Ok(self.pool_admin.remove(PoolAdminRoles::POD_READ_ACCESS))
				}
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
				PoolRole::PODReadAccess => {
					Ok(self.pool_admin.insert(PoolAdminRoles::POD_READ_ACCESS))
				}
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

impl<Now, MinDelay, Moment> PermissionedCurrencyHolders<Now, MinDelay, Moment>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord + Copy,
{
	pub fn empty() -> Self {
		Self::default()
	}

	pub fn is_empty(&self) -> bool {
		self.info.is_none()
	}

	fn validity(&self, delta: Moment) -> Result<Moment, ()> {
		let now: Moment = Now::now().as_secs().into();
		let min_validity = now.saturating_add(MinDelay::get());
		let req_validity = now.saturating_add(delta);

		if req_validity < min_validity {
			return Err(());
		}

		Ok(req_validity)
	}

	pub fn contains(&self) -> bool {
		if let Some(info) = &self.info {
			info.permissioned_till >= Now::now().as_secs().into()
		} else {
			false
		}
	}

	#[allow(clippy::result_unit_err)]
	pub fn remove(&mut self, delta: Moment) -> Result<(), ()> {
		if let Some(info) = &self.info {
			let valid_till = &info.permissioned_till;
			let now = Now::now().as_secs().into();

			if *valid_till <= now {
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
	pub fn insert(&mut self, delta: Moment) -> Result<(), ()> {
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

impl<Now, MinDelay, TrancheId, Moment, MaxTranches>
	TrancheInvestors<Now, MinDelay, TrancheId, Moment, MaxTranches>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord + Copy,
	TrancheId: PartialEq + PartialOrd,
	MaxTranches: Get<u32>,
{
	pub fn empty() -> Self {
		Self::default()
	}

	pub fn is_empty(&self) -> bool {
		self.info.is_empty()
	}

	fn validity(&self, delta: Moment) -> Result<Moment, ()> {
		let now: Moment = Now::now().as_secs().into();
		let min_validity = now.saturating_add(MinDelay::get());
		let req_validity = now.saturating_add(delta);

		if req_validity < min_validity {
			Err(())
		} else {
			Ok(req_validity)
		}
	}

	pub fn contains(&self, tranche: TrancheId) -> bool {
		self.info.iter().any(|info| {
			info.tranche_id == tranche && info.permissioned_till >= Now::now().as_secs().into()
		})
	}

	#[allow(clippy::result_unit_err)]
	pub fn remove(&mut self, tranche: TrancheId, delta: Moment) -> Result<(), ()> {
		if let Some(index) = self.info.iter().position(|info| info.tranche_id == tranche) {
			let valid_till = &self.info[index].permissioned_till;
			let now = Now::now().as_secs().into();

			if *valid_till <= now {
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
	pub fn insert(&mut self, tranche: TrancheId, delta: Moment) -> Result<(), ()> {
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
				})
				.map_err(|_| ())
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
		pub const MaxTranches: u32 = 5;
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
		assert!(PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default().empty());

		let mut roles = PermissionRoles::<Now, MinDelay, TrancheId, MaxTranches>::default();

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
		assert!(roles.add(Role::PoolRole(PoolRole::PODReadAccess)).is_ok());
		assert!(roles.exists(Role::PoolRole(PoolRole::LiquidityAdmin)));
		assert!(roles.exists(Role::PoolRole(PoolRole::MemberListAdmin)));
		assert!(roles.exists(Role::PoolRole(PoolRole::PODReadAccess)));

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
		assert!(roles.rm(Role::PoolRole(PoolRole::PODReadAccess)).is_ok());
		assert!(!roles.exists(Role::PoolRole(PoolRole::LiquidityAdmin)));
		assert!(!roles.exists(Role::PoolRole(PoolRole::MemberListAdmin)));
		assert!(!roles.exists(Role::PoolRole(PoolRole::PODReadAccess)));
	}

	/// Sanity check for every CurrencyId variant's encoding value.
	/// This will stop us from accidentally moving or dropping variants
	/// around which could have silent but serious negative consequences.
	#[test]
	fn currency_id_encode_sanity() {
		use crate::tokens::CurrencyId::*;

		// Verify that every variant encodes to what we would expect it to.
		// If this breaks, we must have changed the order of a variant, added
		// a new variant in between existing variants, or deleted one.
		vec![Native, Tranche(42, [42; 16]), KSM, AUSD, ForeignAsset(89)]
			.into_iter()
			.for_each(|variant| {
				let encoded_u64: Vec<u64> = variant.encode().iter().map(|x| *x as u64).collect();

				assert_eq!(encoded_u64, expected_encoding_value(variant))
			});

		/// Return the expected encoding.
		/// This is useful to force at compile time that we handle all existing variants.
		fn expected_encoding_value(id: crate::tokens::CurrencyId) -> Vec<u64> {
			match id {
				Native => vec![0],
				Tranche(pool_id, tranche_id) => {
					let mut r = vec![1, pool_id, 0, 0, 0, 0, 0, 0, 0];
					r.append(&mut tranche_id.map(|x| x as u64).to_vec());
					r
				}
				KSM => vec![2],
				AUSD => vec![3],
				ForeignAsset(id) => vec![4, id as u64, 0, 0, 0],
			}
		}
	}
}
