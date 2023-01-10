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
use std::marker::PhantomData;

use cfg_primitives::Moment;
use cfg_traits::UpdateState;
use cfg_types::{
	fixed_point::Rate,
	permissions::{PermissionScope, Role},
	tokens::CurrencyId,
};
use frame_support::{
	dispatch::{
		DispatchErrorWithPostInfo, DispatchResult, DispatchResultWithPostInfo, PostDispatchInfo,
	},
	parameter_types,
	traits::{Hooks, SortedMembers},
};
use frame_system::EnsureSigned;
use pallet_pool_system::{pool_types::PoolChanges, tranches::TrancheInput};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::{self as pallet_pool_registry, Config, PoolMutate, WeightInfo};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type TrancheId = [u8; 16];

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Test {
	type AccountData = ();
	type AccountId = u64;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
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
	type SS58Prefix = SS58Prefix;
	type SystemWeightInfo = ();
	type Version = ();
}

impl pallet_timestamp::Config for Test {
	type MinimumPeriod = ();
	type Moment = Moment;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const One: u64 = 1;

	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Moment = 0;

	pub const MaxRoles: u32 = u32::MAX;
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

pub type Balance = u128;

parameter_types! {
	// Pool metadata limit
	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxSizeMetadata: u32 = 100;
}

pub struct ModifyPoolMock<T> {
	phantom: PhantomData<T>,
}

impl<T: Config + pallet_pool_registry::Config> PoolMutate<T::AccountId, T::PoolId>
	for ModifyPoolMock<T>
{
	type Balance = T::Balance;
	type CurrencyId = T::CurrencyId;
	type MaxTokenNameLength = T::MaxTokenNameLength;
	type MaxTokenSymbolLength = T::MaxTokenSymbolLength;
	type MaxTranches = T::MaxTranches;
	type PoolChanges =
		PoolChanges<T::Rate, T::MaxTokenNameLength, T::MaxTokenSymbolLength, T::MaxTranches>;
	type Rate = T::Rate;
	type TrancheInput = TrancheInput<T::Rate, T::MaxTokenNameLength, T::MaxTokenSymbolLength>;

	fn create(
		_admin: T::AccountId,
		_depositor: T::AccountId,
		_pool_id: T::PoolId,
		_tranche_inputs: Vec<Self::TrancheInput>,
		_currency: T::CurrencyId,
		_max_reserve: T::Balance,
		_metadata: Option<Vec<u8>>,
	) -> DispatchResult {
		Ok(())
	}

	fn update(
		_pool_id: T::PoolId,
		_changes: Self::PoolChanges,
	) -> Result<(UpdateState, PostDispatchInfo), DispatchErrorWithPostInfo> {
		Ok((
			UpdateState::Executed,
			Some(T::WeightInfo::update_and_execute(5)).into(),
		))
	}

	fn execute_update(_: T::PoolId) -> DispatchResultWithPostInfo {
		todo!()
	}
}

impl Config for Test {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type InterestRate = Rate;
	type MaxSizeMetadata = MaxSizeMetadata;
	type MaxTokenNameLength = ();
	type MaxTokenSymbolLength = ();
	type MaxTranches = ();
	type ModifyPool = ModifyPoolMock<Self>;
	type Permission = PermissionsMock;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type PoolId = u64;
	type Rate = Rate;
	type RuntimeEvent = RuntimeEvent;
	type TrancheId = TrancheId;
	type WeightInfo = ();
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		PoolRegistry: pallet_pool_registry::{Pallet, Call, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
	}
);

type AccountId = u64;
type PoolId = u64;

pub struct PermissionsMock {}

impl cfg_traits::Permissions<AccountId> for PermissionsMock {
	type Error = sp_runtime::DispatchError;
	type Ok = ();
	type Role = Role;
	type Scope = PermissionScope<PoolId, CurrencyId>;

	fn has(_scope: Self::Scope, _who: AccountId, _role: Self::Role) -> bool {
		true
	}

	fn add(
		_scope: Self::Scope,
		_who: AccountId,
		_role: Self::Role,
	) -> Result<Self::Ok, Self::Error> {
		todo!()
	}

	fn remove(
		_scope: Self::Scope,
		_who: AccountId,
		_role: Self::Role,
	) -> Result<Self::Ok, Self::Error> {
		todo!()
	}
}

// Test externalities builder
//
// This type is mainly used for mocking storage in tests. It is the type alias
// for an in-memory, hashmap-based externalities implementation.
pub struct TestExternalitiesBuilder {}

// Default trait implementation for test externalities builder
impl Default for TestExternalitiesBuilder {
	fn default() -> Self {
		Self {}
	}
}

pub const SECONDS: u64 = 1000;
pub const START_DATE: u64 = 1640995200;

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub fn build(self) -> sp_io::TestExternalities {
		let storage = frame_system::GenesisConfig::default()
			.build_storage::<Test>()
			.unwrap();
		let mut externalities = sp_io::TestExternalities::new(storage);
		externalities.execute_with(|| {
			System::set_block_number(1);
			System::on_initialize(System::block_number());
			Timestamp::on_initialize(System::block_number());
			Timestamp::set(RuntimeOrigin::none(), START_DATE * SECONDS).unwrap();
		});
		externalities
	}
}
