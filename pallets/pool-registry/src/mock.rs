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

use cfg_types::permissions::PermissionScope;
use frame_support::{derive_impl, parameter_types};
use frame_system::EnsureRoot;
use sp_runtime::traits::ConstU32;

use crate::{self as pallet_pool_registry, Config, PoolFeeInput};

pub type Balance = u128;
pub type AccountId = u64;
pub type CurrencyId = u32;
pub type Rate = u16;
pub type PoolId = u8;
pub type TrancheId = [u8; 16];

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Permissions: cfg_mocks::permissions::pallet,
		WriteOffPolicy: cfg_mocks::write_off_policy::pallet,
		PoolSystem: cfg_mocks::pools::pallet,
		PoolRegistry: pallet_pool_registry,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl cfg_mocks::write_off_policy::pallet::Config for Runtime {
	type Policy = ();
	type PoolId = PoolId;
}

impl cfg_mocks::permissions::pallet::Config for Runtime {
	type Scope = PermissionScope<PoolId, CurrencyId>;
}

impl cfg_mocks::pools::pallet::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = ();
	type CurrencyId = CurrencyId;
	type PoolId = PoolId;
	type TrancheId = TrancheId;
}

impl cfg_mocks::pools::pallet::ConfigMut for Runtime {
	type MaxFeesPerPool = ConstU32<5>;
	type MaxTranches = ConstU32<5>;
	type PoolChanges = ();
	type PoolFeeInput = PoolFeeInput<Runtime>;
	type TrancheInput = ();
}

parameter_types! {
	pub const MaxSizeMetadata: u32 = 100;
}

impl Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type InterestRate = Rate;
	type MaxSizeMetadata = MaxSizeMetadata;
	type ModifyPool = PoolSystem;
	type ModifyWriteOffPolicy = WriteOffPolicy;
	type Permission = Permissions;
	type PoolCreateOrigin = EnsureRoot<AccountId>;
	type PoolId = PoolId;
	type RuntimeEvent = RuntimeEvent;
	type TrancheId = TrancheId;
	type WeightInfo = ();
}
