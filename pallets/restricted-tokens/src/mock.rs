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

use cfg_traits::PreConditions;
use frame_support::{derive_impl, parameter_types};
use orml_traits::parameter_type_with_key;
use pallet_restricted_tokens::TransferDetails;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::ConstU32, BuildStorage};
use sp_std::collections::btree_map::BTreeMap;

pub use crate as pallet_restricted_tokens;

pub const DISTR_PER_ACCOUNT: u64 = 1000;
pub type AccountId = u64;
pub type Balance = u64;
pub const POOL_PALLET_ID: AccountId = 999u64;
type Time = u64;
pub const MIN_HOLD_PERIOD: Time = 10;
static mut TIME: Time = 0;
static mut PERIOD_STORAGE: *mut BTreeMap<AccountId, Time> =
	0usize as *mut BTreeMap<AccountId, Time>;
pub const LOCK_ID: [u8; 8] = *b"roc/locs";

struct HoldingPeriodChecker;
impl HoldingPeriodChecker {
	fn get() -> &'static mut BTreeMap<AccountId, Time> {
		unsafe {
			if PERIOD_STORAGE.is_null() {
				let map = Box::new(BTreeMap::<AccountId, Time>::new());
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
	pub fn now() -> Time {
		unsafe { TIME }
	}

	pub fn pass(time: Time) {
		unsafe {
			TIME += time;
		}
	}

	#[allow(dead_code)]
	pub fn set(time: Time) {
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
		use cfg_traits::PreConditions;
		use frame_support::traits::tokens::Preservation;

		use crate::{
			impl_fungibles::*,
			mock::{
				AccountId, Balance, CurrencyId, ExistentialDeposit, RestrictedTokens,
				POOL_PALLET_ID,
			},
			TransferDetails,
		};

		/// Dummy filter, that allows to reduce the balance of native normally
		/// but other balances are only allowed to be reduced by the half of
		/// what is actually reducible.
		///
		/// Additionally, we limit up to the ED for Preservation::Preserve.
		///
		/// NOTE: Since CurrencyId::Cfg is native, this filter passes
		/// CurrencyId::Cfg directly to the fungible::Inspect implementation and
		/// the respective filters.
		pub struct InspectFilter;
		impl PreConditions<FungiblesInspectEffects<CurrencyId, AccountId, Balance>> for InspectFilter {
			type Result = Balance;

			fn check(t: FungiblesInspectEffects<CurrencyId, AccountId, Balance>) -> Self::Result {
				match t {
					FungiblesInspectEffects::ReducibleBalance(
						_asset,
						_who,
						preservation,
						_force,
						actually_reducible,
					) => match preservation {
						// NOTE: This mimics the behavior of the fungible implementation provided by
						// pallet_balances (i.e. withdraw all including ED except for
						// Preservation::Preserve).
						// However, the fungibles implementation by orml_tokens actually behaves
						// slightly differently: It secures ED for Preservation::Protect instead.
						Preservation::Expendable | Preservation::Protect => actually_reducible / 2,
						Preservation::Preserve => {
							actually_reducible / 2 - ExistentialDeposit::get()
						}
					},
				}
			}
		}

		/// Dummmy filter for InspectHold, that does not allow any holding
		/// periods on AUSD and forwards the result of the actual holding period
		/// otherwise.
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
					FungiblesInspectHoldEffects::HoldAvailable(
						asset,
						_who,
						actual_hold_available,
					) => match asset {
						CurrencyId::AUSD => false,
						_ => actual_hold_available,
					},
				}
			}
		}

		/// Dummy filter for Mutate. Allows min and burns normally for all
		/// expect the Restricted-token. This token is only allowed to be
		/// minted/burned into/from the pool-account
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

		/// Dummy filter that enforces hold restrictions given by `CanHold`.
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

		/// Dummy filter for Transfer. Enforces rules for RestrictedTokens
		/// struct on trait level
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

		/// Dummy filter for Unbalanced. Only allows native token actions.
		pub struct UnbalancedFilter;
		impl PreConditions<FungiblesUnbalancedEffects<CurrencyId, AccountId, Balance>>
			for UnbalancedFilter
		{
			type Result = bool;

			fn check(
				t: FungiblesUnbalancedEffects<CurrencyId, AccountId, Balance>,
			) -> Self::Result {
				match t {
					FungiblesUnbalancedEffects::WriteBalance(asset, _, _)
					| FungiblesUnbalancedEffects::SetTotalIssuance(asset, _) => asset == CurrencyId::Cfg,
				}
			}
		}
	}

	pub mod fungible {
		use cfg_traits::PreConditions;
		use frame_support::traits::tokens::Preservation;

		use crate::{
			impl_fungible::*,
			mock::{
				AccountId, Balance, ExistentialDeposit, HoldingPeriodChecker, Timer,
				MIN_HOLD_PERIOD,
			},
		};

		/// Dummy filter, that allows to reduce only till the
		/// ExistentialDeposit for Preservation::Preserve.
		pub struct InspectFilter;
		impl PreConditions<FungibleInspectEffects<AccountId, Balance>> for InspectFilter {
			type Result = Balance;

			fn check(t: FungibleInspectEffects<AccountId, Balance>) -> Self::Result {
				match t {
					FungibleInspectEffects::ReducibleBalance(
						_who,
						preservation,
						_fortitude,
						actually_reducible,
					) => match preservation {
						Preservation::Expendable | Preservation::Protect => actually_reducible,
						// NOTE: If we did not add this extra-check, pallet_balances would still
						// only allow withdrawals up to the ED for `Preserve`.
						Preservation::Preserve => {
							actually_reducible.saturating_sub(ExistentialDeposit::get())
						}
					},
				}
			}
		}

		/// Dummy filter for Transfer. Only allows transfer of native token
		/// after min holding period.
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
		use cfg_traits::PreConditions;
		use frame_support::traits::WithdrawReasons;

		use crate::{
			impl_currency::*,
			mock::{AccountId, Balance},
		};

		/// A dummy filter that ensures that a call to
		/// Currency::ensure_can_withdraw and withdraw result in the expected
		/// behaviour. Especially, it only allows withdraws for
		/// TRANSACTION_PAYMENT reasons.
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
	Encode, Decode, Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Cfg,
	AUSD,
	RestrictedCoin,
}

// Build mock runtime
frame_support::construct_runtime!(
	pub enum Runtime
	{
		System: frame_system,
		OrmlTokens: orml_tokens,
		Balances: pallet_balances,
		Tokens: pallet_restricted_tokens,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type ExistentialDeposit = ExistentialDeposit;
	type RuntimeHoldReason = RuntimeHoldReason;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		match currency_id {
			_ => 1,
		}
	};
}

impl orml_tokens::Config for Runtime {
	type Amount = i64;
	type Balance = Balance;
	type CurrencyHooks = ();
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ConstU32<100>;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub const NativeToken: CurrencyId = CurrencyId::Cfg;
}
impl pallet_restricted_tokens::Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type Fungibles = OrmlTokens;
	type NativeFungible = Balances;
	type NativeToken = NativeToken;
	type PreCurrency = filter::currency::CurrencyFilter;
	type PreExtrTransfer = RestrictedTokens;
	type PreFungibleInspect = filter::fungible::InspectFilter;
	type PreFungibleInspectHold = cfg_traits::Always;
	type PreFungibleMutate = cfg_traits::Always;
	type PreFungibleMutateHold = cfg_traits::Always;
	type PreFungibleTransfer = filter::fungible::TransferFilter;
	type PreFungiblesInspect = filter::fungibles::InspectFilter;
	type PreFungiblesInspectHold = filter::fungibles::InspectHoldFilter;
	type PreFungiblesMutate = filter::fungibles::MutateFilter;
	type PreFungiblesMutateHold = filter::fungibles::MutateHoldFilter;
	type PreFungiblesTransfer = filter::fungibles::TransferFilter;
	type PreFungiblesUnbalanced = filter::fungibles::UnbalancedFilter;
	type PreReservableCurrency = cfg_traits::Always;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeHoldReason = RuntimeHoldReason;
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
	pub fn build(self, optional: Option<impl FnOnce()>) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
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

		orml_tokens::GenesisConfig::<Runtime> { balances }
			.assimilate_storage(&mut storage)
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
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

		let mut ext = sp_io::TestExternalities::from(storage);

		if let Some(execute) = optional {
			ext.execute_with(execute);
		}
		ext
	}
}
