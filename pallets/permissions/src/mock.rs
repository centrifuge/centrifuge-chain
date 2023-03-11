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

use codec::{Decode, Encode, MaxEncodedLen};
pub use dummy::pallet as pallet_dummy;
use frame_support::{
	parameter_types,
	sp_io::TestExternalities,
	sp_runtime::{
		testing::{Header, H256},
		traits::{BlakeTwo256, IdentityLookup},
	},
	traits::{Contains, EitherOfDiverse, Everything, SortedMembers},
	weights::Weight,
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use pallet_permissions::Properties;
use sp_runtime::traits::AccountIdConversion;

///! Mock environment setup for testing the pallet-permissions
use crate::{self as pallet_permissions};

#[derive(codec::Encode, codec::Decode, scale_info::TypeInfo, Debug, Clone, Eq, PartialEq)]
pub enum OrganisationRole {
	SeniorExeutive,
	HeadOfSaubermaching,
	Admin,
}

#[derive(codec::Encode, codec::Decode, scale_info::TypeInfo, Debug, Clone, Eq, PartialEq)]
pub enum XcmRole {
	Sender,
	Receiver,
}

#[derive(codec::Encode, codec::Decode, scale_info::TypeInfo, Debug, Clone, Eq, PartialEq)]
pub enum Role {
	Organisation(OrganisationRole),
	Xcm(XcmRole),
}

bitflags::bitflags! {
		/// The current admin roles we support
		#[derive(codec::Encode, codec::Decode, scale_info::TypeInfo, MaxEncodedLen)]
		pub struct OrgStorage: u32 {
			const SENIOR_EXEC = 0b00000001;
			const HEAD_OF_SAUBERMACHING  = 0b00000010;
					const ADMIN = 0b00000100;
		}
}

bitflags::bitflags! {
		/// The current admin roles we support
		#[derive(codec::Encode, codec::Decode, scale_info::TypeInfo, MaxEncodedLen)]
		pub struct XcmStorage: u32 {
			const SENDER = 0b00000001;
			const RECEIVER  = 0b00000010;
		}
}

#[derive(
	codec::Encode, codec::Decode, scale_info::TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen,
)]
pub struct Storage {
	org: OrgStorage,
	xcm: XcmStorage,
}

impl Default for Storage {
	fn default() -> Self {
		Self {
			org: OrgStorage::empty(),
			xcm: XcmStorage::empty(),
		}
	}
}

#[derive(
	codec::Encode, codec::Decode, scale_info::TypeInfo, Debug, Clone, Eq, PartialEq, MaxEncodedLen,
)]
pub enum Scope {
	PalletA,
	PalletB,
}

impl Default for Scope {
	fn default() -> Self {
		Self::PalletA
	}
}

impl Properties for Storage {
	type Error = ();
	type Ok = ();
	type Property = Role;

	fn exists(&self, property: Self::Property) -> bool {
		match property {
			Role::Xcm(role) => match role {
				XcmRole::Receiver => self.xcm.contains(XcmStorage::RECEIVER),
				XcmRole::Sender => self.xcm.contains(XcmStorage::SENDER),
			},
			Role::Organisation(role) => match role {
				OrganisationRole::SeniorExeutive => self.org.contains(OrgStorage::SENIOR_EXEC),
				OrganisationRole::HeadOfSaubermaching => {
					self.org.contains(OrgStorage::HEAD_OF_SAUBERMACHING)
				}
				OrganisationRole::Admin => self.org.contains(OrgStorage::ADMIN),
			},
		}
	}

	fn empty(&self) -> bool {
		self.org.is_empty() && self.xcm.is_empty()
	}

	fn rm(&mut self, property: Self::Property) -> Result<(), ()> {
		match property {
			Role::Xcm(role) => match role {
				XcmRole::Receiver => self.xcm.remove(XcmStorage::RECEIVER),
				XcmRole::Sender => self.xcm.remove(XcmStorage::SENDER),
			},
			Role::Organisation(role) => match role {
				OrganisationRole::SeniorExeutive => self.org.remove(OrgStorage::SENIOR_EXEC),
				OrganisationRole::HeadOfSaubermaching => {
					self.org.remove(OrgStorage::HEAD_OF_SAUBERMACHING)
				}
				OrganisationRole::Admin => self.org.remove(OrgStorage::ADMIN),
			},
		};
		Ok(())
	}

	fn add(&mut self, property: Self::Property) -> Result<(), ()> {
		match property {
			Role::Xcm(role) => match role {
				XcmRole::Receiver => self.xcm.insert(XcmStorage::RECEIVER),
				XcmRole::Sender => self.xcm.insert(XcmStorage::SENDER),
			},
			Role::Organisation(role) => match role {
				OrganisationRole::SeniorExeutive => self.org.insert(OrgStorage::SENIOR_EXEC),
				OrganisationRole::HeadOfSaubermaching => {
					self.org.insert(OrgStorage::HEAD_OF_SAUBERMACHING)
				}
				OrganisationRole::Admin => self.org.insert(OrgStorage::ADMIN),
			},
		};
		Ok(())
	}
}

mod dummy {
	#[frame_support::pallet]
	pub mod pallet {
		use frame_support::pallet_prelude::*;
		use frame_system::{ensure_signed, pallet_prelude::OriginFor};

		use crate::Permissions;

		/// Configure the pallet by specifying the parameters and types on which it depends.
		#[pallet::config]
		pub trait Config: frame_system::Config {
			type Scope: Member + Parameter;

			type Role: Member + Parameter;

			type Permission: Permissions<
				Self::AccountId,
				Scope = Self::Scope,
				Role = Self::Role,
				Error = DispatchError,
			>;

			#[pallet::constant]
			type PalletId: Get<Self::AccountId>;
		}

		#[pallet::error]
		pub enum Error<T> {
			AlreadyCleared,
			NotCleared,
		}

		#[pallet::pallet]
		#[pallet::generate_store(pub(super) trait Store)]
		pub struct Pallet<T>(_);

		#[pallet::call]
		impl<T: Config> Pallet<T> {
			#[pallet::weight(100)]
			#[pallet::call_index(0)]
			pub fn test_add(
				origin: OriginFor<T>,
				scope: T::Scope,
				role: T::Role,
			) -> DispatchResult {
				let who = ensure_signed(origin)?;

				ensure!(
					!T::Permission::has(scope.clone(), who.clone(), role.clone()),
					Error::<T>::AlreadyCleared
				);

				T::Permission::add(scope, who, role)?;

				Ok(())
			}

			#[pallet::weight(100)]
			#[pallet::call_index(1)]
			pub fn test_rm(origin: OriginFor<T>, scope: T::Scope, role: T::Role) -> DispatchResult {
				let who = ensure_signed(origin)?;

				ensure!(
					T::Permission::has(scope.clone(), who.clone(), role.clone()),
					Error::<T>::NotCleared
				);

				T::Permission::remove(scope, who, role)?;

				Ok(())
			}
		}
	}
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;
pub type AccountId = u64;

// Build mock runtime
frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Permissions: pallet_permissions::{Pallet, Call, Storage, Event<T>},
		Dummy: pallet_dummy::{Pallet, Call}
	}
);

// Parameterize frame system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
			pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024).set_proof_size(u64::MAX).into());
}

// Implement frame system configuration for the mock runtime
impl frame_system::Config for Runtime {
	type AccountData = ();
	type AccountId = AccountId;
	type BaseCallFilter = Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = BlockWeights;
	type DbWeight = ();
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = ();
	type SystemWeightInfo = ();
	type Version = ();
}

parameter_types! {
	pub const One: u64 = 1;
	pub const MaxRoles: u32 = 10;
	pub const MaxTranches: u32 = 5;
}

type AdminOrigin = EitherOfDiverse<EnsureRoot<u64>, EnsureSignedBy<One, u64>>;

impl pallet_permissions::Config for Runtime {
	type AdminOrigin = AdminOrigin;
	type Editors = Editors;
	type MaxRolesPerScope = MaxRoles;
	type MaxTranches = MaxTranches;
	type Role = Role;
	type RuntimeEvent = RuntimeEvent;
	type Scope = Scope;
	type Storage = Storage;
	type WeightInfo = ();
}

parameter_types! {
	pub const DummyAccount: AccountId = 100;
	pub const FailingDummy: AccountId = 200;
}

pub struct WrapperAccount(u64);
impl AccountIdConversion<AccountId> for WrapperAccount {
	fn into_sub_account_truncating<S: Encode>(&self, _sub: S) -> AccountId {
		self.0
	}

	fn try_into_sub_account<S: Encode>(&self, _sub: S) -> Option<AccountId> {
		todo!()
	}

	fn try_from_sub_account<S: Decode>(_x: &AccountId) -> Option<(Self, S)> {
		None
	}
}

pub struct Editors;
impl Contains<(AccountId, Option<Role>, Scope, Role)> for Editors {
	fn contains(t: &(AccountId, Option<Role>, Scope, Role)) -> bool {
		let (account, with_role, _scope, role) = t;
		let dummy = DummyAccount::get();

		match (account, role, with_role) {
			(_, _, Some(Role::Organisation(OrganisationRole::Admin))) => true,
			(1, _, _) => true,
			(x, role, _) if *x == dummy => match role {
				Role::Xcm(xcm) => match xcm {
					XcmRole::Receiver => true,
					XcmRole::Sender => false,
				},
				Role::Organisation(_) => true,
			},
			_ => false,
		}
	}
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

impl pallet_dummy::Config for Runtime {
	type PalletId = DummyAccount;
	type Permission = Permissions;
	type Role = Role;
	type Scope = Scope;
}

#[derive(Default)]
pub struct TestExternalitiesBuilder;

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub fn build(self, optional: impl FnOnce()) -> TestExternalities {
		let storage = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		let mut ext = TestExternalities::from(storage);
		ext.execute_with(optional);
		ext
	}
}
