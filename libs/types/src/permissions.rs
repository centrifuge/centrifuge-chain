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

use codec::{Decode, Encode};
use frame_support::{
	sp_runtime::traits::Saturating,
	traits::{Get, UnixTime},
};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::{
	cmp::{Ord, PartialEq, PartialOrd},
	marker::PhantomData,
	vec::Vec,
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
	pub pool_admin: PoolAdminRoles,
	pub currency_admin: CurrencyAdminRoles,
	pub permissioned_asset_holder: PermissionedCurrencyHolders<Now, MinDelay, Moment>,
	pub tranche_investor: TrancheInvestors<Now, MinDelay, TrancheId, Moment>,
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
			Ok(self.info.push(TrancheInvestorInfo {
				tranche_id: tranche,
				permissioned_till: validity,
			}))
		}
	}
}

#[cfg(test)]
mod tests {
	///! Tests for some types in the common section for our runtimes
	use super::*;

	/// Sanity check for every CurrencyId variant's encoding value.
	/// This will stop us from accidentally moving or dropping variants
	/// around which could have silent but serious negative consequences.
	#[test]
	fn currency_id_encode_sanity() {
		use crate::CurrencyId::*;

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
		fn expected_encoding_value(id: crate::CurrencyId) -> Vec<u64> {
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
