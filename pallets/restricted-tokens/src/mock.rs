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

pub use crate as pallet_restricted_tokens;
use common_traits::PreConditions;
use common_types::Moment;
use frame_support::parameter_types;
use frame_support::sp_io::TestExternalities;
use frame_support::traits::{Everything, GenesisBuild};
use orml_traits::parameter_type_with_key;
use pallet_restricted_tokens::TransferDetails;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::testing::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use sp_std::collections::btree_map::BTreeMap;

pub const DISTR_PER_ACCOUNT: u64 = 1000;
pub type AccountId = u64;
pub type Balance = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;
pub const POOL_PALLET_ID: AccountId = 999u64;
pub const MIN_HOLD_PERIOD: Moment = 10;
static mut TIME: Moment = 0;
static mut PERIOD_STORAGE: *mut BTreeMap<AccountId, Moment> =
	0usize as *mut BTreeMap<AccountId, Moment>;
pub const LOCK_ID: [u8; 8] = *b"roc/locs";

struct HoldingPeriodChecker;
impl HoldingPeriodChecker {
	fn get() -> &'static mut BTreeMap<AccountId, Moment> {
		unsafe {
			if PERIOD_STORAGE.is_null() {
				let map = Box::new(BTreeMap::<AccountId, Moment>::new());
				PERIOD_STORAGE = Box::into_raw(map);

				&mut *(PERIOD_STORAGE)
			} else {
				&mut *(PERIOD_STORAGE)
			}
		}
	}
}

pub struct Timer;
impl Timer {
	pub fn now() -> Moment {
		unsafe { TIME }
	}

	pub fn pass(time: Moment) {
		unsafe {
			TIME += time;
		}
	}

	#[allow(dead_code)]
	pub fn set(time: Moment) {
		unsafe {
			TIME = time;
		}
	}

	#[allow(dead_code)]
	pub fn reset() {
		Timer::set(0);
	}
}

mod filter {
	pub mod fungibles {
		use crate::impl_fungibles::*;
		use crate::mock::{AccountId, Balance, CurrencyId, RestrictedTokens, POOL_PALLET_ID};
		use crate::TransferDetails;
		use common_traits::PreConditions;

		/// Dummy filter, that allows to reduce the balance of native normally
		/// but other balances are only allowed to be reduced by the half of
		/// what is actually reducible.
		pub struct InspectFilter;
		impl PreConditions<FungiblesInspectEffects<CurrencyId, AccountId, Balance>> for InspectFilter {
			type Result = Balance;

			fn check(t: FungiblesInspectEffects<CurrencyId, AccountId, Balance>) -> Self::Result {
				match t {
					FungiblesInspectEffects::ReducibleBalance(
						asset,
						_who,
						_keep_alive,
						actually_reducible,
					) => {
						match asset {
							// Note this filter actually never filters CurrencyId::Cfg. As CFG is the native one, which is passe
							// directly to the fungible::Inspect implementation and the respective filters.
							_ => actually_reducible / 2,
						}
					}
				}
			}
		}

		/// Dummmy filter for InspectHold, that does not allow any holding periods on AUSD and
		/// forwards the result of the actual holding period otherwise.
		pub struct InspectHoldFilter;
		impl PreConditions<FungiblesInspectHoldEffects<CurrencyId, AccountId, Balance>>
			for InspectHoldFilter
		{
			type Result = bool;

			fn check(
				t: FungiblesInspectHoldEffects<CurrencyId, AccountId, Balance>,
			) -> Self::Result {
				match t {
					FungiblesInspectHoldEffects::CanHold(
						asset,
						_who,
						_amount,
						can_actually_hold,
					) => match asset {
						CurrencyId::AUSD => false,
						_ => can_actually_hold,
					},
				}
			}
		}

		/// Dummy filter for Mutate. Allows min and burns normally for all expect the Restricted-token.
		/// This token is only allowed to be minted/burned into/from the pool-account
		pub struct MutateFilter;
		impl PreConditions<FungiblesMutateEffects<CurrencyId, AccountId, Balance>> for MutateFilter {
			type Result = bool;

			fn check(t: FungiblesMutateEffects<CurrencyId, AccountId, Balance>) -> Self::Result {
				match t {
					FungiblesMutateEffects::BurnFrom(asset, who, _amount) => match asset {
						CurrencyId::RestrictedCoin => match who {
							_x if who == POOL_PALLET_ID => true,
							_ => false,
						},
						_ => true,
					},
					FungiblesMutateEffects::MintInto(asset, who, _amount) => match asset {
						CurrencyId::RestrictedCoin => match who {
							_x if who == POOL_PALLET_ID => true,
							_ => false,
						},
						_ => true,
					},
				}
			}
		}

		/// Dummy filter that enforeces hold restrictens given by can hold.
		pub struct MutateHoldFilter;
		impl PreConditions<FungiblesMutateHoldEffects<CurrencyId, AccountId, Balance>>
			for MutateHoldFilter
		{
			type Result = bool;

			fn check(
				t: FungiblesMutateHoldEffects<CurrencyId, AccountId, Balance>,
			) -> Self::Result {
				match t {
					FungiblesMutateHoldEffects::Hold(currency, who, amount) => {
						InspectHoldFilter::check(FungiblesInspectHoldEffects::CanHold(
							currency, who, amount, true,
						))
					}
					_ => true,
				}
			}
		}

		/// Dummy filter for Transfer. Enforces rules for RestrictedTokens struct on trait level
		pub struct TransferFilter;
		impl PreConditions<FungiblesTransferEffects<CurrencyId, AccountId, Balance>> for TransferFilter {
			type Result = bool;

			fn check(t: FungiblesTransferEffects<CurrencyId, AccountId, Balance>) -> Self::Result {
				match t {
					FungiblesTransferEffects::Transfer(
						currency,
						send,
						recv,
						amount,
						_keep_alive,
					) => {
						let details = TransferDetails::new(send, recv, currency, amount);

						RestrictedTokens::check(details)
					}
				}
			}
		}
	}

	pub mod fungible {
		use crate::impl_fungible::*;
		use crate::mock::{
			AccountId, Balance, ExistentialDeposit, HoldingPeriodChecker, Timer, MIN_HOLD_PERIOD,
		};
		use common_traits::PreConditions;

		/// Dummy filter, that allows to reduce only till the ExistentialDeposit.
		pub struct InspectFilter;
		impl PreConditions<FungibleInspectEffects<AccountId, Balance>> for InspectFilter {
			type Result = Balance;

			fn check(t: FungibleInspectEffects<AccountId, Balance>) -> Self::Result {
				match t {
					FungibleInspectEffects::ReducibleBalance(
						_who,
						keep_alive,
						actually_reducible,
					) => {
						if keep_alive {
							actually_reducible
						} else {
							actually_reducible.saturating_sub(ExistentialDeposit::get())
						}
					}
				}
			}
		}

		/// Dummy filter for Transfer. Only allows transfer of native token after min holding period.
		pub struct TransferFilter;
		impl PreConditions<FungibleTransferEffects<AccountId, Balance>> for TransferFilter {
			type Result = bool;

			fn check(t: FungibleTransferEffects<AccountId, Balance>) -> Self::Result {
				match t {
					FungibleTransferEffects::Transfer(send, recv, _amount, _keep_alive) => {
						if let Some(sender_recv) = HoldingPeriodChecker::get().get(&send) {
							let now = Timer::now();

							if now >= *sender_recv + MIN_HOLD_PERIOD {
								HoldingPeriodChecker::get().remove(&send);
								HoldingPeriodChecker::get().insert(recv, now);

								true
							} else {
								false
							}
						} else {
							false
						}
					}
				}
			}
		}
	}

	pub mod currency {
		use crate::impl_currency::*;
		use crate::mock::{AccountId, Balance};
		use common_traits::PreConditions;
		use frame_support::traits::WithdrawReasons;

		/// A dummy filter that ensures that a call to Currency::ensure_can_withdraw and
		/// withdraw result in the expected behaviour. Especially, it only allows
		/// withdraws for TRANSACTION_PAYMENT reasons.
		pub struct CurrencyFilter;
		impl PreConditions<CurrencyEffects<AccountId, Balance>> for CurrencyFilter {
			type Result = bool;

			fn check(t: CurrencyEffects<AccountId, Balance>) -> Self::Result {
				match t {
					CurrencyEffects::EnsureCanWithdraw(
						_who,
						_amount,
						reason,
						_new_balance,
						result,
					) => reason.contains(WithdrawReasons::TRANSACTION_PAYMENT) && result.is_ok(),
					CurrencyEffects::Withdraw(_who, _amount, reason, _liveness) => {
						reason.contains(WithdrawReasons::TRANSACTION_PAYMENT)
					}
					_ => true,
				}
			}
		}
	}
}

#[derive(
	codec::Encode,
	codec::Decode,
	Clone,
	Copy,
	Debug,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	scale_info::TypeInfo,
	codec::MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Cfg,
	AUSD,
	RestrictedCoin,
}

// Build mock runtime
frame_support::construct_runtime!(
	pub enum MockRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		OrmlTokens: orml_tokens::{Pallet, Config<T>, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Config<T>, Storage, Event<T>},
		Tokens: pallet_restricted_tokens::{Pallet, Call, Event<T>},
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
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		// every currency has a zero existential deposit
		match currency_id {
			_ => 1,
		}
	};
}

parameter_types! {
	pub const MaxLocks: u32 = 100;
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for MockRuntime {
	type MaxLocks = MaxLocks;
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

parameter_types! {
	pub const MaxReserves: u32 = 50;
}

impl orml_tokens::Config for MockRuntime {
	type Event = Event;
	type Balance = Balance;
	type Amount = i64;
	type CurrencyId = CurrencyId;
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

parameter_types! {
	pub const NativeToken: CurrencyId = CurrencyId::Cfg;
}
impl pallet_restricted_tokens::Config for MockRuntime {
	type Event = Event;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type PreExtrTransfer = RestrictedTokens;
	type PreFungiblesInspect = filter::fungibles::InspectFilter;
	type PreFungiblesInspectHold = filter::fungibles::InspectHoldFilter;
	type PreFungiblesMutate = filter::fungibles::MutateFilter;
	type PreFungiblesMutateHold = filter::fungibles::MutateHoldFilter;
	type PreFungiblesTransfer = filter::fungibles::TransferFilter;
	type Fungibles = OrmlTokens;
	type PreCurrency = filter::currency::CurrencyFilter;
	type PreReservableCurrency = common_traits::Always;
	type PreFungibleInspect = filter::fungible::InspectFilter;
	type PreFungibleInspectHold = common_traits::Always;
	type PreFungibleMutate = common_traits::Always;
	type PreFungibleMutateHold = common_traits::Always;
	type PreFungibleTransfer = filter::fungible::TransferFilter;
	type NativeFungible = Balances;
	type NativeToken = NativeToken;
	type WeightInfo = ();
}

// Restricted coins are only allowed to be send to users with an id over 100
pub struct RestrictedTokens;
impl PreConditions<TransferDetails<AccountId, CurrencyId, Balance>> for RestrictedTokens {
	type Result = bool;

	fn check(t: TransferDetails<AccountId, CurrencyId, Balance>) -> bool {
		match t.id {
			CurrencyId::AUSD => true,
			CurrencyId::RestrictedCoin => t.recv >= 100 && t.send >= 100,
			CurrencyId::Cfg => true,
		}
	}
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
		let ausd = (0..10)
			.into_iter()
			.map(|idx| (idx, CurrencyId::AUSD, DISTR_PER_ACCOUNT))
			.collect::<Vec<(AccountId, CurrencyId, Balance)>>();
		let restric_1 = (0..10)
			.into_iter()
			.map(|idx| (idx, CurrencyId::RestrictedCoin, DISTR_PER_ACCOUNT))
			.collect::<Vec<(AccountId, CurrencyId, Balance)>>();
		let restric_2 = (100..200)
			.into_iter()
			.map(|idx| (idx, CurrencyId::RestrictedCoin, DISTR_PER_ACCOUNT))
			.collect::<Vec<(AccountId, CurrencyId, Balance)>>();

		let mut balances = vec![];
		balances.extend(ausd);
		balances.extend(restric_1);
		balances.extend(restric_2);

		orml_tokens::GenesisConfig::<MockRuntime> { balances }
			.assimilate_storage(&mut storage)
			.unwrap();

		pallet_balances::GenesisConfig::<MockRuntime> {
			balances: (0..10u64)
				.into_iter()
				.map(|idx| {
					HoldingPeriodChecker::get().insert(idx, Timer::now());
					(idx, DISTR_PER_ACCOUNT)
				})
				.collect(),
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut ext = TestExternalities::from(storage);

		if let Some(execute) = optional {
			ext.execute_with(execute);
		}
		ext
	}
}
