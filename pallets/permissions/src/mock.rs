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

///! Mock environment setup for testing the pallet-permissions
use crate::{self as pallet_permissions};

mod dummy {
	#[frame_support::pallet]
	pub mod pallet {
		use frame_support::pallet_prelude::*;
		use frame_system::ensure_signed;
		use Permissions;

		/// Configure the pallet by specifying the parameters and types on which it depends.
		#[pallet::config]
		pub trait Config: frame_system::Config {
			type Location: Member + Parameter;

			type Role: Member + Parameter;

			type Permission: Permissions<
				T::AccountId,
				Location = Self::Location,
				Role = Self::Role,
				Error = DispatchError,
			>;
		}

		#[pallet::Error]
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
			pub fn test_add(
				origin: OriginFor<T>,
				location: T::Location,
				role: T::Role,
			) -> DispatchResult {
				let who = ensure_signed(origin);

				ensure!(
					!Self::Permission::clearance(location, who, role),
					Error::<T>::AlreadyCleared
				);

				Self::Permission::add_permission(location, who, role)?;

				Ok(())
			}

			pub fn test_rm(
				origin: OriginFor<T>,
				location: T::Location,
				role: T::Role,
			) -> DispatchResult {
				let who = ensure_signed(origin);

				ensure!(
					Self::Permission::clearance(location, who, role),
					Error::<T>::NotCleared
				);

				Self::Permission::rm_permission(location, who, role)?;

				Ok(())
			}
		}
	}
}

use dummy::pallet::Pallet as pallet_dummy;
use frame_support::sp_io::TestExternalities;
use frame_support::sp_runtime::testing::{Header, H256};
use frame_support::sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use frame_support::traits::Everything;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Balance = u128;
type Block = frame_system::mocking::MockBlock<MockRuntime>;

// Build mock runtime
frame_support::construct_runtime!(
	pub enum MockRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Permissions: pallet_permissions::{Pallet, Call, Config, Storage, Event<T>}
		Dummy: pallet_dummy::{Pallet, Call, Config, Event<T>}
	}
);

// Parameterize frame system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(1024);
}

// Implement frame system configuration for the mock runtime
impl frame_system::Config for MockRuntime {
	type BaseCallFilter = Everything;
	type BlockWeights = BlockWeights;
	type BlockLength = ();
	type Origin = Origin;
	type Index = u64;
	type Call = Call;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}

pub struct TestExternalitiesBuilder;

// Implement default trait for test externalities builder
impl Default for TestExternalitiesBuilder {
	fn default() -> Self {
		Self {}
	}
}

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub fn build(self, optional: Option<impl FnOnce()>) -> TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<MockRuntime>()
			.unwrap();

		let mut ext = TestExternalities::from(storage);

		if let Some(execute) = optional {
			ext.execute_with(execute);
		}
		ext
	}
}
