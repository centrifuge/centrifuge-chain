// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Tests for Utility Pallet

#![cfg(test)]

use frame_support::{
	assert_err_ignore_postinfo, assert_noop, assert_ok,
	dispatch::{DispatchError, DispatchErrorWithPostInfo, Dispatchable, Pays},
	error::BadOrigin,
	parameter_types, storage,
	traits::{ConstU32, ConstU64, Contains, GenesisBuild},
	weights::Weight,
};
use pallet_collective::{EnsureProportionAtLeast, Instance1};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, Hash, IdentityLookup},
	TokenError,
};

use super::*;
use crate as utility;

type BlockNumber = u64;

// example module to test behaviors.
#[frame_support::pallet(dev_mode)]
pub mod example {
	use frame_support::{dispatch::WithPostDispatchInfo, pallet_prelude::*};
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(*_weight)]
		pub fn noop(_origin: OriginFor<T>, _weight: Weight) -> DispatchResult {
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(*_start_weight)]
		pub fn foobar(
			origin: OriginFor<T>,
			err: bool,
			_start_weight: Weight,
			end_weight: Option<Weight>,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;
			if err {
				let error: DispatchError = "The cake is a lie.".into();
				if let Some(weight) = end_weight {
					Err(error.with_weight(weight))
				} else {
					Err(error)?
				}
			} else {
				Ok(end_weight.into())
			}
		}

		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn big_variant(_origin: OriginFor<T>, _arg: [u8; 400]) -> DispatchResult {
			Ok(())
		}
	}
}

mod mock_democracy {
	pub use pallet::*;
	#[frame_support::pallet(dev_mode)]
	pub mod pallet {
		use frame_support::pallet_prelude::*;
		use frame_system::pallet_prelude::*;

		#[pallet::pallet]
		pub struct Pallet<T>(_);

		#[pallet::config]
		pub trait Config: frame_system::Config + Sized {
			type RuntimeEvent: From<Event<Self>>
				+ IsType<<Self as frame_system::Config>::RuntimeEvent>;
			type ExternalMajorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		}

		#[pallet::call]
		impl<T: Config> Pallet<T> {
			#[pallet::call_index(3)]
			#[pallet::weight(0)]
			pub fn external_propose_majority(origin: OriginFor<T>) -> DispatchResult {
				T::ExternalMajorityOrigin::ensure_origin(origin)?;
				Self::deposit_event(Event::<T>::ExternalProposed);
				Ok(())
			}
		}

		#[pallet::event]
		#[pallet::generate_deposit(pub(super) fn deposit_event)]
		pub enum Event<T: Config> {
			ExternalProposed,
		}
	}
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Call, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		RootTesting: pallet_root_testing::{Pallet, Call, Storage},
		Council: pallet_collective::<Instance1>,
		Utility: utility::{Pallet, Call, Event},
		Example: example::{Pallet, Call},
		Democracy: mock_democracy::{Pallet, Call, Event<T>},
	}
);

parameter_types! {
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(Weight::MAX);
}
impl frame_system::Config for Test {
	type AccountData = pallet_balances::AccountData<u64>;
	type AccountId = u64;
	type BaseCallFilter = TestBaseCallFilter;
	type BlockHashCount = ConstU64<250>;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = BlockWeights;
	type DbWeight = ();
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = ();
	type SystemWeightInfo = frame_system::weights::SubstrateWeight<Test>;
	type Version = ();
}

impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = u64;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU64<1>;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type RuntimeHoldReason = ();
	type WeightInfo = ();
}

impl pallet_root_testing::Config for Test {}

impl pallet_timestamp::Config for Test {
	type MinimumPeriod = ConstU64<3>;
	type Moment = u64;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

const MOTION_DURATION_IN_BLOCKS: BlockNumber = 3;
parameter_types! {
	pub const MultisigDepositBase: u64 = 1;
	pub const MultisigDepositFactor: u64 = 1;
	pub const MaxSignatories: u32 = 3;
	pub const MotionDuration: BlockNumber = MOTION_DURATION_IN_BLOCKS;
	pub const MaxProposals: u32 = 100;
	pub const MaxMembers: u32 = 100;
	pub MaxProposalWeight: Weight = sp_runtime::Perbill::from_percent(50) * BlockWeights::get().max_block;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Test {
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type MaxMembers = MaxMembers;
	type MaxProposalWeight = MaxProposalWeight;
	type MaxProposals = MaxProposals;
	type MotionDuration = MotionDuration;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SetMembersOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

impl example::Config for Test {}

pub struct TestBaseCallFilter;
impl Contains<RuntimeCall> for TestBaseCallFilter {
	fn contains(c: &RuntimeCall) -> bool {
		match *c {
			// Transfer works. Use `transfer_keep_alive` for a call that doesn't pass the filter.
			RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death { .. }) => true,
			RuntimeCall::Utility(_) => true,
			// For benchmarking, this acts as a noop call
			RuntimeCall::System(frame_system::Call::remark { .. }) => true,
			// For tests
			RuntimeCall::Example(_) => true,
			// For council origin tests.
			RuntimeCall::Democracy(_) => true,
			_ => false,
		}
	}
}
impl mock_democracy::Config for Test {
	type ExternalMajorityOrigin = EnsureProportionAtLeast<u64, Instance1, 3, 4>;
	type RuntimeEvent = RuntimeEvent;
}
impl Config for Test {
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

type ExampleCall = example::Call<Test>;
type UtilityCall = crate::Call<Test>;

use frame_system::Call as SystemCall;
use pallet_balances::Call as BalancesCall;
use pallet_root_testing::Call as RootTestingCall;
use pallet_timestamp::Call as TimestampCall;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![(1, 10), (2, 10), (3, 10), (4, 10), (5, 2)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	pallet_collective::GenesisConfig::<Test, Instance1> {
		members: vec![1, 2, 3],
		phantom: Default::default(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

fn call_transfer(dest: u64, value: u64) -> RuntimeCall {
	RuntimeCall::Balances(BalancesCall::transfer_allow_death { dest, value })
}

fn call_foobar(err: bool, start_weight: Weight, end_weight: Option<Weight>) -> RuntimeCall {
	RuntimeCall::Example(ExampleCall::foobar {
		err,
		start_weight,
		end_weight,
	})
}

#[test]
fn remark_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Balances::free_balance(1), 10);
		assert_eq!(Balances::free_balance(2), 10);
		assert_ok!(Utility::remark(
			RuntimeOrigin::signed(1),
			vec![call_transfer(2, 5), call_transfer(2, 5)]
		),);
		assert_eq!(Balances::free_balance(1), 0);
		assert_eq!(Balances::free_balance(2), 20);
	});
}

#[test]
fn remark_revert() {
	new_test_ext().execute_with(|| {
		let call = call_transfer(2, 5);
		let info = call.get_dispatch_info();

		assert_eq!(Balances::free_balance(1), 10);
		assert_eq!(Balances::free_balance(2), 10);
		let remark_calls = RuntimeCall::Utility(crate::Call::<Test>::remark {
			calls: vec![
				call_transfer(2, 5),
				call_transfer(2, 10),
				call_transfer(2, 5),
			],
		});
		assert_noop!(
			remark_calls.dispatch(RuntimeOrigin::signed(1)),
			DispatchErrorWithPostInfo {
				post_info: PostDispatchInfo {
					actual_weight: Some(<Test as Config>::WeightInfo::remark(2) + info.weight * 2),
					pays_fee: Pays::Yes
				},
				error: TokenError::FundsUnavailable.into(),
			}
		);
		assert_eq!(Balances::free_balance(1), 10);
		assert_eq!(Balances::free_balance(2), 10);
	});
}

#[test]
fn remark_handles_weight_refund() {
	new_test_ext().execute_with(|| {
		let start_weight = Weight::from_parts(100, 0);
		let end_weight = Weight::from_parts(75, 0);
		let diff = start_weight - end_weight;
		let batch_len = 4;

		// Full weight when ok
		let inner_call = call_foobar(false, start_weight, None);
		let batch_calls = vec![inner_call; batch_len as usize];
		let call = RuntimeCall::Utility(UtilityCall::remark { calls: batch_calls });
		let info = call.get_dispatch_info();
		let result = call.dispatch(RuntimeOrigin::signed(1));
		assert_ok!(result);
		assert_eq!(extract_actual_weight(&result, &info), info.weight);

		// Refund weight when ok
		let inner_call = call_foobar(false, start_weight, Some(end_weight));
		let batch_calls = vec![inner_call; batch_len as usize];
		let call = RuntimeCall::Utility(UtilityCall::remark { calls: batch_calls });
		let info = call.get_dispatch_info();
		let result = call.dispatch(RuntimeOrigin::signed(1));
		assert_ok!(result);
		// Diff is refunded
		assert_eq!(
			extract_actual_weight(&result, &info),
			info.weight - diff * batch_len
		);

		// Full weight when err
		let good_call = call_foobar(false, start_weight, None);
		let bad_call = call_foobar(true, start_weight, None);
		let batch_calls = vec![good_call, bad_call];
		let call = RuntimeCall::Utility(UtilityCall::remark { calls: batch_calls });
		let info = call.get_dispatch_info();
		let result = call.dispatch(RuntimeOrigin::signed(1));
		assert_err_ignore_postinfo!(result, "The cake is a lie.");
		// No weight is refunded
		assert_eq!(extract_actual_weight(&result, &info), info.weight);

		// Refund weight when err
		let good_call = call_foobar(false, start_weight, Some(end_weight));
		let bad_call = call_foobar(true, start_weight, Some(end_weight));
		let batch_calls = vec![good_call, bad_call];
		let batch_len = batch_calls.len() as u64;
		let call = RuntimeCall::Utility(UtilityCall::remark { calls: batch_calls });
		let info = call.get_dispatch_info();
		let result = call.dispatch(RuntimeOrigin::signed(1));
		assert_err_ignore_postinfo!(result, "The cake is a lie.");
		assert_eq!(
			extract_actual_weight(&result, &info),
			info.weight - diff * batch_len
		);

		// Partial batch completion
		let good_call = call_foobar(false, start_weight, Some(end_weight));
		let bad_call = call_foobar(true, start_weight, Some(end_weight));
		let batch_calls = vec![good_call, bad_call.clone(), bad_call];
		let call = RuntimeCall::Utility(UtilityCall::remark { calls: batch_calls });
		let info = call.get_dispatch_info();
		let result = call.dispatch(RuntimeOrigin::signed(1));
		assert_err_ignore_postinfo!(result, "The cake is a lie.");
		assert_eq!(
			extract_actual_weight(&result, &info),
			// Real weight is 2 calls at end_weight
			<Test as Config>::WeightInfo::remark(2) + end_weight * 2,
		);
	});
}

#[test]
fn remark_does_not_nest() {
	new_test_ext().execute_with(|| {
		let remark = RuntimeCall::Utility(UtilityCall::remark {
			calls: vec![
				call_transfer(2, 1),
				call_transfer(2, 1),
				call_transfer(2, 1),
			],
		});

		let info = remark.get_dispatch_info();

		assert_eq!(Balances::free_balance(1), 10);
		assert_eq!(Balances::free_balance(2), 10);
		// A nested remark call will not pass the filter, and fail with `BadOrigin`.
		assert_noop!(
			Utility::remark(RuntimeOrigin::signed(1), vec![remark.clone()]),
			DispatchErrorWithPostInfo {
				post_info: PostDispatchInfo {
					actual_weight: Some(<Test as Config>::WeightInfo::remark(1) + info.weight),
					pays_fee: Pays::Yes
				},
				error: frame_system::Error::<Test>::CallFiltered.into(),
			}
		);

		// And for those who want to get a little fancy, we check that the filter
		// persists across other kinds of dispatch wrapping functions... in this case
		// `remark(batch(remark(..)))`
		let batch_nested = RuntimeCall::Utility(UtilityCall::batch {
			calls: vec![remark],
		});
		// Batch will end with `Ok`, but does not actually execute as we can see from
		// the event and balances.
		assert_ok!(Utility::remark(
			RuntimeOrigin::signed(1),
			vec![batch_nested]
		));
		System::assert_has_event(
			utility::Event::BatchInterrupted {
				index: 0,
				error: frame_system::Error::<Test>::CallFiltered.into(),
			}
			.into(),
		);
		assert_eq!(Balances::free_balance(1), 10);
		assert_eq!(Balances::free_balance(2), 10);
	});
}

#[test]
fn batch_limit() {
	new_test_ext().execute_with(|| {
		let calls = vec![RuntimeCall::System(SystemCall::remark { remark: vec![] }); 40_000];
		assert_noop!(
			Utility::batch(RuntimeOrigin::signed(1), calls.clone()),
			Error::<Test>::TooManyCalls
		);
		assert_noop!(
			Utility::remark(RuntimeOrigin::signed(1), calls),
			Error::<Test>::TooManyCalls
		);
	});
}
