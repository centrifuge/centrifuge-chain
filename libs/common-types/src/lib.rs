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

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]

///! Common-types of the Centrifuge chain.
use codec::{Decode, Encode};
use common_traits::{AssetProperties, Properties};
use frame_support::scale_info::build::Fields;
use frame_support::scale_info::Path;
use frame_support::scale_info::Type;
use frame_support::sp_runtime::traits::Saturating;
use frame_support::traits::{Get, UnixTime};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::Zero;
use sp_runtime::Perquintill;
use sp_std::cmp::{Ord, PartialEq, PartialOrd};
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;

// Pub exports
pub use tokens::*;

pub mod ids;
#[cfg(test)]
mod tests;
mod tokens;

/// PoolId type we use.
pub type PoolId = u64;

/// A representation of a tranche identifier
pub type TrancheId = [u8; 16];

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

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug)]
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
	#[derive(codec::Encode, codec::Decode,  TypeInfo)]
	pub struct PoolAdminRoles: u32 {
		const POOL_ADMIN = 0b00000001;
		const BORROWER  = 0b00000010;
		const PRICING_ADMIN = 0b00000100;
		const LIQUIDITY_ADMIN = 0b00001000;
		const MEMBER_LIST_ADMIN = 0b00010000;
		const RISK_ADMIN = 0b00100000;
	}

	/// The current admin roles we support
	#[derive(codec::Encode, codec::Decode,  TypeInfo)]
	pub struct CurrencyAdminRoles: u32 {
		const PERMISSIONED_ASSET_MANAGER = 0b00000001;
		const PERMISSIONED_ASSET_ISSUER  = 0b00000010;
	}
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq)]
pub struct PermissionedCurrencyHolderInfo<Moment> {
	permissioned_till: Moment,
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq)]
pub struct TrancheInvestorInfo<TrancheId, Moment> {
	tranche_id: TrancheId,
	permissioned_till: Moment,
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq)]
pub struct PermissionedCurrencyHolders<Now, MinDelay, Moment> {
	info: Option<PermissionedCurrencyHolderInfo<Moment>>,
	_phantom: PhantomData<(Now, MinDelay)>,
}

#[derive(Encode, Decode, TypeInfo, Debug, Clone, Eq, PartialEq)]
pub struct TrancheInvestors<Now, MinDelay, TrancheId, Moment> {
	info: Vec<TrancheInvestorInfo<TrancheId, Moment>>,
	_phantom: PhantomData<(Now, MinDelay)>,
}

/// The structure that we store in the pallet-permissions storage
/// This here implements trait Properties.
#[derive(Encode, Decode, TypeInfo, Clone, Eq, PartialEq, Debug)]
pub struct PermissionRoles<Now, MinDelay, TrancheId, Moment = u64> {
	pool_admin: PoolAdminRoles,
	currency_admin: CurrencyAdminRoles,
	permissioned_asset_holder: PermissionedCurrencyHolders<Now, MinDelay, Moment>,
	tranche_investor: TrancheInvestors<Now, MinDelay, TrancheId, Moment>,
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

impl<Now, MinDelay, TrancheId, Moment> Default
	for TrancheInvestors<Now, MinDelay, TrancheId, Moment>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord,
	TrancheId: PartialEq + PartialOrd,
{
	fn default() -> Self {
		Self {
			info: Vec::default(),
			_phantom: Default::default(),
		}
	}
}

impl<Now, MinDelay, TrancheId, Moment> Default for PermissionRoles<Now, MinDelay, TrancheId, Moment>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord,
	TrancheId: PartialEq + PartialOrd,
{
	fn default() -> Self {
		Self {
			pool_admin: PoolAdminRoles::empty(),
			currency_admin: CurrencyAdminRoles::empty(),
			permissioned_asset_holder:
				PermissionedCurrencyHolders::<Now, MinDelay, Moment>::default(),
			tranche_investor: TrancheInvestors::<Now, MinDelay, TrancheId, Moment>::default(),
		}
	}
}

/// The implementation of trait Properties for our PermissionsRoles does not care which Moment
/// is passed to the PoolRole::TrancheInvestor(TrancheId, Moment) variant.
/// This UNION shall reflect that and explain to the reader why it is passed here.
pub const UNION: u64 = 0;

impl<Now, MinDelay, TrancheId, Moment> Properties
	for PermissionRoles<Now, MinDelay, TrancheId, Moment>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord + Copy,
	TrancheId: PartialEq + PartialOrd,
{
	type Property = Role<TrancheId, Moment>;
	type Error = ();
	type Ok = ();

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

impl<Now, MinDelay, TrancheId, Moment> TrancheInvestors<Now, MinDelay, TrancheId, Moment>
where
	Now: UnixTime,
	MinDelay: Get<Moment>,
	Moment: From<u64> + PartialEq + PartialOrd + Saturating + Ord + Copy,
	TrancheId: PartialEq + PartialOrd,
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
		self.info
			.iter()
			.position(|info| {
				info.tranche_id == tranche && info.permissioned_till >= Now::now().as_secs().into()
			})
			.is_some()
	}

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

	pub fn insert(&mut self, tranche: TrancheId, delta: Moment) -> Result<(), ()> {
		let validity = self.validity(delta)?;

		if let Some(index) = self.info.iter().position(|info| info.tranche_id == tranche) {
			if self.info[index].permissioned_till > validity {
				Err(())
			} else {
				Ok(self.info[index].permissioned_till = validity)
			}
		} else {
			Ok(self.info.push(TrancheInvestorInfo {
				tranche_id: tranche,
				permissioned_till: validity,
			}))
		}
	}
}

/// A struct we need as the pallets implementing trait Time
/// do not implement TypeInfo. This wraps this and implements everything manually.
#[derive(Encode, Decode, Eq, PartialEq, Debug, Clone)]
pub struct TimeProvider<T>(PhantomData<T>);

impl<T> UnixTime for TimeProvider<T>
where
	T: UnixTime,
{
	fn now() -> core::time::Duration {
		<T as UnixTime>::now()
	}
}

impl<T> TypeInfo for TimeProvider<T> {
	type Identity = ();

	fn type_info() -> Type {
		Type::builder()
			.path(Path::new("TimeProvider", module_path!()))
			.docs(&["A wrapper around a T that provides a trait Time implementation. Should be filtered out."])
			.composite(Fields::unit())
	}
}

/// A representation of a pool identifier that can be converted to an account address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolLocator<PoolId> {
	pub pool_id: PoolId,
}

// Type that indicates a point in time
pub use common_traits::Moment;

pub enum Adjustment<Amount> {
	Increase(Amount),
	Decrease(Amount),
}

/// A representation of a pool identifier that can be converted to an account address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct AssetAccount<AssetId> {
	pub asset_id: AssetId,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct AssetInfo<AccountId, Currency, AssetId> {
	pub owner: AccountId,
	pub id: AssetId,
	pub payment_currency: Currency,
}

impl<AccountId, Currency, AssetId> AssetProperties<AccountId>
	for AssetInfo<AccountId, Currency, AssetId>
where
	AccountId: Clone,
	Currency: Clone,
	AssetId: Clone,
{
	type Currency = Currency;
	type Id = AssetId;

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

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct TotalOrder<Balance> {
	pub invest: Balance,
	pub redeem: Balance,
}

impl<Balance: Zero> Default for TotalOrder<Balance> {
	fn default() -> Self {
		TotalOrder {
			invest: Zero::zero(),
			redeem: Zero::zero(),
		}
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct FulfillmentWithPrice<BalanceRatio> {
	pub invest: Perquintill,
	pub redeem: Perquintill,
	pub price: BalanceRatio,
}
