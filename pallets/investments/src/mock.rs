// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use std::ops::Add;

pub use cfg_primitives::CFG as CURRENCY;
use cfg_primitives::*;
use cfg_traits::{investments::OrderManager, PreConditions};
use cfg_types::{
	fixed_point::Quantity,
	investments::{InvestmentAccount, InvestmentInfo},
	orders::{FulfillmentWithPrice, TotalOrder},
	tokens::CurrencyId,
};
use frame_support::{
	derive_impl,
	dispatch::DispatchResultWithPostInfo,
	parameter_types,
	traits::{
		tokens::{
			fungibles::{Inspect, Mutate},
			Fortitude, Precision, Preservation,
		},
		Nothing,
	},
};
use orml_traits::GetByKey;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_arithmetic::{FixedPointNumber, Perquintill};
use sp_io::TestExternalities;
use sp_runtime::{traits::AccountIdConversion, BuildStorage, DispatchError, DispatchResult};
use sp_std::{
	cell::RefCell,
	collections::btree_map::BTreeMap,
	convert::{TryFrom, TryInto},
	rc::Rc,
};

pub use crate as pallet_investments;

pub type AccountId = u64;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Investments: pallet_investments,
		OrmlTokens: orml_tokens,
		Balances: pallet_balances,
		MockAccountant: cfg_mocks::pallet_mock_pools,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

parameter_types! {
	pub MaxLocks: u32 = 2;
	pub const MaxReserves: u32 = 50;
}

impl GetByKey<CurrencyId, u128> for ExistentialDeposit {
	fn get(_: &CurrencyId) -> u128 {
		ExistentialDeposit::get()
	}
}

impl orml_tokens::Config for Runtime {
	type Amount = i64;
	type Balance = Balance;
	type CurrencyHooks = ();
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = Nothing;
	type ExistentialDeposits = ExistentialDeposit;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 1;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type RuntimeHoldReason = ();
}

impl cfg_mocks::pallet_mock_pools::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Quantity;
	type CurrencyId = CurrencyId;
	type PoolId = PoolId;
	type TrancheCurrency = InvestmentId;
	type TrancheId = TrancheId;
}

pub struct NoopCollectHook;
impl cfg_traits::StatusNotificationHook for NoopCollectHook {
	type Error = sp_runtime::DispatchError;
	type Id = (AccountId, InvestmentId);
	type Status = cfg_types::investments::CollectedAmount<Balance, Balance>;

	fn notify_status_change(_id: Self::Id, _status: Self::Status) -> DispatchResult {
		Ok(())
	}
}

parameter_types! {
	pub const MaxOutstandingCollect: u32 = 10;
}

impl pallet_investments::Config for Runtime {
	type Accountant = MockAccountant;
	type Amount = Balance;
	type BalanceRatio = Quantity;
	type CollectedInvestmentHook = NoopCollectHook;
	type CollectedRedemptionHook = NoopCollectHook;
	type InvestmentId = InvestmentId;
	type MaxOutstandingCollects = MaxOutstandingCollect;
	type PreConditions = Always;
	type RuntimeEvent = RuntimeEvent;
	type Tokens = OrmlTokens;
	type WeightInfo = ();
}

pub struct Always;
impl<T> PreConditions<T> for Always {
	type Result = DispatchResult;

	fn check(_: T) -> Self::Result {
		Ok(())
	}
}

// TODO: This struct should be temporarily needed only
//       We should add the possibility to use subsets of the
//       global CurrencyId enum
#[derive(
	Copy,
	Clone,
	Encode,
	Decode,
	PartialEq,
	Debug,
	Ord,
	PartialOrd,
	Eq,
	TypeInfo,
	Serialize,
	Deserialize,
	MaxEncodedLen,
)]
pub enum InvestmentId {
	PoolTranche {
		pool_id: PoolId,
		tranche_id: TrancheId,
	},
}

impl Default for InvestmentId {
	fn default() -> Self {
		Self::PoolTranche {
			pool_id: Default::default(),
			tranche_id: Default::default(),
		}
	}
}

impl From<InvestmentId> for CurrencyId {
	fn from(val: InvestmentId) -> Self {
		match val {
			InvestmentId::PoolTranche {
				pool_id,
				tranche_id,
			} => CurrencyId::Tranche(pool_id, tranche_id),
		}
	}
}

// Test externalities builder
//
// This type is mainly used for mocking storage in tests. It is the type alias
// for an in-memory, hashmap-based externalities implementation.
pub struct TestExternalitiesBuilder;

parameter_types! {
	pub const InvestorA: AccountId = 1;
	pub const InvestorB: AccountId = 2;
	pub const InvestorC: AccountId = 3;
	pub const InvestorD: AccountId = 4;
	pub const TrancheHolderA: AccountId = 11;
	pub const TrancheHolderB: AccountId = 12;
	pub const TrancheHolderC: AccountId = 13;
	pub const TrancheHolderD: AccountId = 14;
	pub const Owner: AccountId = 100;
}

/// The pool id we use for tests
pub const POOL_ID: PoolId = 0;
/// One tranche id of our test pool
pub const TRANCHE_ID_0: [u8; 16] = [0u8; 16];
/// The second tranche id of our test pool
pub const TRANCHE_ID_1: [u8; 16] = [1u8; 16];
/// The amount the owner of the known investments has at the start
pub const OWNER_START_BALANCE: u128 = 100_000_000 * CURRENCY;

/// The investment-id for investing into pool 0 and tranche 0
pub const INVESTMENT_0_0: InvestmentId = InvestmentId::PoolTranche {
	pool_id: POOL_ID,
	tranche_id: TRANCHE_ID_0,
};
/// The investment-id for investing into pool 0 and tranche 1
pub const INVESTMENT_0_1: InvestmentId = InvestmentId::PoolTranche {
	pool_id: POOL_ID,
	tranche_id: TRANCHE_ID_1,
};

/// An unknown investment id -> i.e. a not yet created pool
pub const UNKNOWN_INVESTMENT: InvestmentId = InvestmentId::PoolTranche {
	pool_id: 1,
	tranche_id: TRANCHE_ID_0,
};

/// The currency id for the AUSD token
pub const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub(crate) fn build() -> TestExternalities {
		let mut storage = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: vec![
				// Owner holds enough capital to satisfy redemptions
				(
					Owner::get(),
					AUSD_CURRENCY_ID,
					OWNER_START_BALANCE + ExistentialDeposit::get(),
				),
				(
					InvestorA::get(),
					AUSD_CURRENCY_ID,
					100 * CURRENCY + ExistentialDeposit::get(),
				),
				(
					InvestorB::get(),
					AUSD_CURRENCY_ID,
					100 * CURRENCY + ExistentialDeposit::get(),
				),
				(
					InvestorC::get(),
					AUSD_CURRENCY_ID,
					100 * CURRENCY + ExistentialDeposit::get(),
				),
				(
					InvestorD::get(),
					AUSD_CURRENCY_ID,
					100 * CURRENCY + ExistentialDeposit::get(),
				),
				(
					TrancheHolderA::get(),
					INVESTMENT_0_0.into(),
					100 * CURRENCY + ExistentialDeposit::get(),
				),
				(
					TrancheHolderB::get(),
					INVESTMENT_0_0.into(),
					100 * CURRENCY + ExistentialDeposit::get(),
				),
				(
					TrancheHolderC::get(),
					INVESTMENT_0_0.into(),
					100 * CURRENCY + ExistentialDeposit::get(),
				),
				(
					TrancheHolderD::get(),
					INVESTMENT_0_0.into(),
					100 * CURRENCY + ExistentialDeposit::get(),
				),
			],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut externalities = TestExternalities::new(storage);
		externalities.execute_with(|| {
			// We need to set this, otherwise on genesis (i.e. 0)
			// no events are stored
			System::set_block_number(1);

			// Mocked behaviour for the accountant
			configure_accountant_mock();
		});

		externalities
	}
}

pub fn configure_accountant_mock() {
	let state = Rc::new(RefCell::new(BTreeMap::from([
		(
			INVESTMENT_0_0,
			InvestmentInfo {
				owner: Owner::get(),
				id: INVESTMENT_0_0,
				payment_currency: AUSD_CURRENCY_ID,
			},
		),
		(
			INVESTMENT_0_1,
			InvestmentInfo {
				owner: Owner::get(),
				id: INVESTMENT_0_1,
				payment_currency: AUSD_CURRENCY_ID,
			},
		),
	])));

	fn get<E: Clone>(
		state: &Rc<RefCell<BTreeMap<InvestmentId, E>>>,
		id: InvestmentId,
	) -> Result<E, DispatchError> {
		state
			.borrow()
			.get(&id)
			.cloned()
			.ok_or(DispatchError::Other("Not found"))
	}

	MockAccountant::mock_info({
		let state = state.clone();
		move |id| get(&state, id)
	});

	MockAccountant::mock_balance(|id, who| OrmlTokens::balance(id.into(), who));

	MockAccountant::mock_transfer({
		let state = state.clone();
		move |id, source, dest, amount| {
			let _ = get(&state, id)?;
			<OrmlTokens as Mutate<AccountId>>::transfer(
				id.into(),
				source,
				dest,
				amount,
				Preservation::Expendable,
			)
			.map(|_| ())
		}
	});

	MockAccountant::mock_InvestmentAccountant_deposit({
		let state = state.clone();
		move |buyer, id, amount| {
			let _ = get(&state, id)?;
			<OrmlTokens as Mutate<AccountId>>::mint_into(id.into(), buyer, amount).map(|_| ())
		}
	});

	MockAccountant::mock_InvestmentAccountant_withdraw({
		let state = state.clone();
		move |seller, id, amount| {
			let _ = get(&state, id)?;
			<OrmlTokens as Mutate<AccountId>>::burn_from(
				id.into(),
				seller,
				amount,
				Precision::Exact,
				Fortitude::Polite,
			)
			.map(|_| ())
		}
	});
}

pub(crate) fn last_event() -> RuntimeEvent {
	let events = frame_system::Pallet::<Runtime>::events();
	// compare to the last event record
	let frame_system::EventRecord { event, .. } = &events[events.len().saturating_sub(1)];
	event.clone()
}

pub(crate) fn n_last_event(n: usize) -> RuntimeEvent {
	let events = frame_system::Pallet::<Runtime>::events();
	// compare to the last event record
	let frame_system::EventRecord { event, .. } = &events[events.len().saturating_sub(n + 1)];
	event.clone()
}

pub(crate) fn investment_account(investment_id: InvestmentId) -> AccountId {
	InvestmentAccount { investment_id }.into_account_truncating()
}

pub(crate) fn free_balance_of(who: AccountId, currency_id: CurrencyId) -> Balance {
	<orml_tokens::Pallet<Runtime> as orml_traits::MultiCurrency<AccountId>>::free_balance(
		currency_id,
		&who,
	)
}

/// Invest amount into INVESTMENT_0_0
///
/// User accounts are the default Investor{A,B,C}
pub(crate) fn invest_x_per_investor(amount: Balance) -> DispatchResult {
	Investments::update_invest_order(
		RuntimeOrigin::signed(InvestorA::get()),
		INVESTMENT_0_0,
		amount,
	)?;
	Investments::update_invest_order(
		RuntimeOrigin::signed(InvestorB::get()),
		INVESTMENT_0_0,
		amount,
	)?;
	Investments::update_invest_order(
		RuntimeOrigin::signed(InvestorC::get()),
		INVESTMENT_0_0,
		amount,
	)
}

/// Redeem amount into INVESTMENT_0_0
///
/// User accounts are the default TrancheHolder{A,B,C}
pub(crate) fn redeem_x_per_investor(amount: Balance) -> DispatchResult {
	Investments::update_redeem_order(
		RuntimeOrigin::signed(TrancheHolderA::get()),
		INVESTMENT_0_0,
		amount,
	)?;
	Investments::update_redeem_order(
		RuntimeOrigin::signed(TrancheHolderB::get()),
		INVESTMENT_0_0,
		amount,
	)?;
	Investments::update_redeem_order(
		RuntimeOrigin::signed(TrancheHolderC::get()),
		INVESTMENT_0_0,
		amount,
	)
}

/// Create a Rate type. Where full is the non-decimal value and decimal value us
/// defined by dec_n/dec_d
///
/// # E.g.
/// ```rust
/// use cfg_primitives::Balance;
/// use cfg_types::fixed_point::Quantity;
///
/// let rate = crate::mock::price_of(3, 5, 100);
/// assert_eq!(rate.into_inner(), 3050000000000000000000000000) // I.e. price is 3,05
/// ```
pub(crate) fn price_of(full: Balance, dec_n: Balance, dec_d: Balance) -> Quantity {
	let full = Quantity::from_inner(Quantity::DIV.saturating_mul(full));
	let decimals = Quantity::saturating_from_rational(dec_n, dec_d);

	full.add(decimals)
}

/// Creates a fulfillment of given perc and price
pub(crate) fn fulfillment_of(perc: Perquintill, price: Quantity) -> FulfillmentWithPrice<Quantity> {
	FulfillmentWithPrice {
		of_amount: perc,
		price,
	}
}

/// Fulfills the given fulfillment for INVESTMENT_0_0 on both invest and redeem
/// side
pub(crate) fn fulfill_x(fulfillment: FulfillmentWithPrice<Quantity>) -> DispatchResult {
	fulfill_invest_x(fulfillment)?;
	fulfill_redeem_x(fulfillment)
}

/// Fulfills the given fulfillment for INVESTMENT_0_0 on the investment side
pub(crate) fn fulfill_invest_x(fulfillment: FulfillmentWithPrice<Quantity>) -> DispatchResult {
	let _invest_orders = Investments::process_invest_orders(INVESTMENT_0_0)?;
	Investments::invest_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Fulfills the given fulfillment for INVESTMENT_0_0 on the investment side
pub(crate) fn fulfill_redeem_x(fulfillment: FulfillmentWithPrice<Quantity>) -> DispatchResult {
	let _redeem_orders = Investments::process_redeem_orders(INVESTMENT_0_0)?;
	Investments::redeem_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Invest 50 * CURRENCY per Investor into INVESTMENT_0_0 and fulfills
/// the given fulfillment.
pub(crate) fn invest_fulfill_x(fulfillment: FulfillmentWithPrice<Quantity>) -> DispatchResult {
	invest_x_per_investor(50 * CURRENCY)?;

	let _invest_orders = Investments::process_invest_orders(INVESTMENT_0_0)?;
	Investments::invest_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Invest given amount per Investor into INVESTMENT_0_0 and fulfills
/// the given fulfillment.
pub(crate) fn invest_x_fulfill_x(
	invest_per_investor: Balance,
	fulfillment: FulfillmentWithPrice<Quantity>,
) -> DispatchResult {
	invest_x_per_investor(invest_per_investor)?;

	let _invest_orders = Investments::process_invest_orders(INVESTMENT_0_0)?;
	Investments::invest_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Invest given amount per Investor into INVESTMENT_0_0 and fulfills
/// the given fulfillment.
pub(crate) fn invest_x_per_fulfill_x(
	invest_per_investor: Vec<(AccountId, Balance)>,
	fulfillment: FulfillmentWithPrice<Quantity>,
) -> DispatchResult {
	for (who, amount) in invest_per_investor {
		Investments::update_invest_order(RuntimeOrigin::signed(who), INVESTMENT_0_0, amount)?;
	}
	let _invest_orders = Investments::process_invest_orders(INVESTMENT_0_0)?;
	Investments::invest_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Invest given amount per Investor into INVESTMENT_0_0, run the given closure
/// and fulfills the given fulfillment.
pub(crate) fn invest_x_runner_fulfill_x<F>(
	invest_per_investor: Balance,
	fulfillment: FulfillmentWithPrice<Quantity>,
	runner: F,
) -> DispatchResult
where
	F: FnOnce(TotalOrder<Balance>) -> DispatchResult,
{
	invest_x_per_investor(invest_per_investor)?;
	let invest_orders = Investments::process_invest_orders(INVESTMENT_0_0)?;
	runner(invest_orders)?;
	Investments::invest_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Redeem 50 * CURRENCY per TrancheHolder into INVESTMENT_0_0 and fulfills
/// the given fulfillment.
pub(crate) fn redeem_fulfill_x(fulfillment: FulfillmentWithPrice<Quantity>) -> DispatchResult {
	redeem_x_per_investor(50 * CURRENCY)?;

	let _redeem_orders = Investments::process_redeem_orders(INVESTMENT_0_0);
	Investments::redeem_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Redeem given amount per TrancheHolder into INVESTMENT_0_0 and fulfills
/// the given fulfillment.
pub(crate) fn redeem_x_fulfill_x(
	redeem_per_investor: Balance,
	fulfillment: FulfillmentWithPrice<Quantity>,
) -> DispatchResult {
	redeem_x_per_investor(redeem_per_investor)?;

	let _redeem_orders = Investments::process_redeem_orders(INVESTMENT_0_0);
	Investments::redeem_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Invest given amount per Investor into INVESTMENT_0_0 and fulfills
/// the given fulfillment.
pub(crate) fn redeem_x_per_fulfill_x(
	redeem_per_investor: Vec<(AccountId, Balance)>,
	fulfillment: FulfillmentWithPrice<Quantity>,
) -> DispatchResult {
	for (who, amount) in redeem_per_investor {
		Investments::update_redeem_order(RuntimeOrigin::signed(who), INVESTMENT_0_0, amount)?;
	}
	let _redeem_orders = Investments::process_redeem_orders(INVESTMENT_0_0)?;
	Investments::redeem_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Redeem given amount per TrancheHolder into INVESTMENT_0_0, run the given
/// closure and fulfills the given fulfillment.
pub(crate) fn redeem_x_runner_fulfill_x<F>(
	redeem_per_investor: Balance,
	fulfillment: FulfillmentWithPrice<Quantity>,
	runner: F,
) -> DispatchResult
where
	F: FnOnce(TotalOrder<Balance>) -> DispatchResult,
{
	redeem_x_per_investor(redeem_per_investor)?;
	let redeem_orders = Investments::process_redeem_orders(INVESTMENT_0_0)?;
	runner(redeem_orders)?;
	Investments::redeem_fulfillment(INVESTMENT_0_0, fulfillment)
}

/// Collect both invest and redemptions
pub(crate) fn collect_both(
	who: RuntimeOrigin,
	investment_id: InvestmentId,
) -> DispatchResultWithPostInfo {
	Investments::collect_investments(who.clone(), investment_id)?;
	Investments::collect_redemptions(who, investment_id)
}
