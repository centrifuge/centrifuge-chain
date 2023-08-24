// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.
//
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.
//
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::{currency_decimals, parachains, AccountId, Balance, PoolId, TrancheId, CFG};
use cfg_traits::{
	investments::{OrderManager, TrancheCurrency as TrancheCurrencyT},
	liquidity_pools::InboundQueue,
};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Rate,
	investments::{InvestCollection, InvestmentAccount, RedeemCollection},
	orders::FulfillmentWithPrice,
	permissions::{PermissionScope, PoolRole, Role, UNION},
	pools::TrancheMetadata,
	tokens::{
		CrossChainTransferability, CurrencyId, CurrencyId::ForeignAsset, CustomMetadata,
		ForeignAssetId,
	},
};
use development_runtime::{
	Balances, ForeignInvestments, Investments, LiquidityPools, OrmlAssetRegistry, OrmlTokens,
	Permissions, Runtime as DevelopmentRuntime, RuntimeOrigin, System,
};
use frame_support::{
	assert_noop, assert_ok,
	traits::{fungible::Mutate as _, fungibles::Mutate, Get, PalletInfo},
};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use pallet_foreign_investments::types::{InnerRedeemState, InvestState, RedeemState};
use pallet_investments::CollectOutcome;
use runtime_common::account_conversion::AccountConverter;
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, ConstU32, Convert, EnsureAdd, One, Zero},
	BoundedVec, DispatchError, Perquintill, SaturatedConversion, WeakBoundedVec,
};
use xcm_emulator::TestExt;

use crate::{
	liquidity_pools::pallet::development::{
		setup::{dollar, ALICE, BOB},
		test_net::{Development, Moonbeam, RelayChain, TestNet},
		tests::liquidity_pools::{
			non_foreign_investments::setup::{
				do_initial_increase_investment, do_initial_increase_redemption,
			},
			setup::{
				asset_metadata, create_ausd_pool, create_currency_pool,
				enable_liquidity_pool_transferability,
				investments::{
					default_investment_account, default_investment_id, default_tranche_id,
					general_currency_index, investment_id,
				},
				setup_pre_requirements, LiquidityPoolMessage, DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				DEFAULT_POOL_ID, DEFAULT_VALIDITY,
			},
		},
	},
	utils::AUSD_CURRENCY_ID,
};

#[test]
fn inbound_increase_invest_order() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();
		let pool_id = DEFAULT_POOL_ID;
		let amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;

		// Create new pool
		create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial investment
		do_initial_increase_investment(pool_id, amount, investor, currency_id);

		// Verify the order was updated to the amount
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_invest_order(
				default_investment_id(),
			)
			.amount,
			amount
		);
	});
}

#[test]
fn inbound_decrease_invest_order() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();
		let pool_id = DEFAULT_POOL_ID;
		let invest_amount: u128 = 100_000_000;
		let decrease_amount = invest_amount / 3;
		let final_amount = invest_amount - decrease_amount;
		let investor: AccountId = BOB.into();
		let currency_id: CurrencyId = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;

		// Create new pool
		create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial investment
		do_initial_increase_investment(pool_id, invest_amount, investor.clone(), currency_id);

		// Mock incoming decrease message
		let msg = LiquidityPoolMessage::DecreaseInvestOrder {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
			amount: decrease_amount,
		};

		// Expect failure if transferability is disabled since this is required for
		// preparing the `ExecutedDecreaseInvest` message.
		assert_noop!(
			LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotLiquidityPoolsTransferable
		);
		enable_liquidity_pool_transferability(currency_id);

		// Execute byte message
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Verify investment was decreased into investment account
		assert_eq!(
			OrmlTokens::free_balance(currency_id, &default_investment_account()),
			final_amount
		);
		// Since the investment was done in the pool currency, the decrement happens
		// synchronously and thus it must be burned from investor's holdings
		assert_eq!(OrmlTokens::free_balance(currency_id, &investor), 0);
		assert!(System::events().iter().any(|e| e.event
			== pallet_investments::Event::<DevelopmentRuntime>::InvestOrderUpdated {
				investment_id: default_investment_id(),
				submitted_at: 0,
				who: investor.clone(),
				amount: final_amount
			}
			.into()));
		assert!(System::events().iter().any(|e| e.event
			== orml_tokens::Event::<DevelopmentRuntime>::Withdrawn {
				currency_id,
				who: investor.clone(),
				amount: decrease_amount
			}
			.into()));
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_invest_order(
				default_investment_id(),
			)
			.amount,
			final_amount
		);
	});
}

#[test]
fn inbound_cancel_invest_order() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();
		let pool_id = DEFAULT_POOL_ID;
		let invest_amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;

		// Create new pool
		create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial investment
		do_initial_increase_investment(pool_id, invest_amount, investor.clone(), currency_id);

		// Verify investment account holds funds before cancelling
		assert_eq!(
			OrmlTokens::free_balance(currency_id, &default_investment_account()),
			invest_amount
		);

		// Mock incoming cancel message
		let msg = LiquidityPoolMessage::CancelInvestOrder {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
		};

		// Expect failure if transferability is disabled since this is required for
		// preparing the `ExecutedDecreaseInvest` message.
		assert_noop!(
			LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotLiquidityPoolsTransferable
		);
		enable_liquidity_pool_transferability(currency_id);

		// Execute byte message
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Foreign InvestmentState should be cleared
		assert!(!pallet_foreign_investments::InvestmentState::<
			DevelopmentRuntime,
		>::contains_key(&investor, default_investment_id()));
		assert!(System::events().iter().any(|e| {
			e.event == pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignInvestmentCleared {
					investor: investor.clone(),
					investment_id: default_investment_id(),
				}
				.into()
		}));

		// Verify investment was entirely drained from investment account
		assert_eq!(
			OrmlTokens::free_balance(currency_id, &default_investment_account()),
			0
		);
		// Since the investment was done in the pool currency, the decrement happens
		// synchronously and thus it must be burned from investor's holdings
		assert_eq!(OrmlTokens::free_balance(currency_id, &investor), 0);
		assert!(System::events().iter().any(|e| e.event
			== pallet_investments::Event::<DevelopmentRuntime>::InvestOrderUpdated {
				investment_id: default_investment_id(),
				submitted_at: 0,
				who: investor.clone(),
				amount: 0
			}
			.into()));
		assert!(System::events().iter().any(|e| e.event
			== orml_tokens::Event::<DevelopmentRuntime>::Withdrawn {
				currency_id,
				who: investor.clone(),
				amount: invest_amount
			}
			.into()));
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_invest_order(
				default_investment_id(),
			)
			.amount,
			0
		);
	});
}

#[test]
fn inbound_collect_invest_order() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();
		let pool_id = DEFAULT_POOL_ID;
		let amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;
		let sending_domain_locator = Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());

		// Create new pool
		create_currency_pool(pool_id, currency_id, currency_decimals.into());
		let investment_currency_id: CurrencyId = default_investment_id().into();

		// Set permissions and execute initial investment
		do_initial_increase_investment(pool_id, amount, investor.clone(), currency_id);
		let events_before_collect = System::events();

		// Process and fulfill order
		// NOTE: Without this step, the order id is not cleared and
		// `Event::InvestCollectedForNonClearedOrderId` be dispatched
		assert_ok!(Investments::process_invest_orders(default_investment_id()));

		// Tranche tokens will be minted upon fulfillment
		assert_eq!(OrmlTokens::total_issuance(investment_currency_id), 0);
		assert_ok!(Investments::invest_fulfillment(
			default_investment_id(),
			FulfillmentWithPrice::<Rate> {
				of_amount: Perquintill::one(),
				price: Rate::one(),
			}
		));
		assert_eq!(OrmlTokens::total_issuance(investment_currency_id), amount);

		// Mock collection message msg
		let msg = LiquidityPoolMessage::CollectInvest {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
		};

		// Execute byte message
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Remove events before collect execution
		let events_since_collect: Vec<_> = System::events()
			.into_iter()
			.filter(|e| !events_before_collect.contains(e))
			.collect();

		// Verify investment was transferred to the domain locator
		assert_eq!(
			OrmlTokens::free_balance(default_investment_id().into(), &sending_domain_locator),
			amount
		);

		// Order should have been cleared by fulfilling investment
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_invest_order(
				default_investment_id(),
			)
			.amount,
			0
		);
		assert!(!events_since_collect.iter().any(|e| {
			e.event
				== pallet_investments::Event::<DevelopmentRuntime>::InvestCollectedForNonClearedOrderId {
					investment_id: default_investment_id(),
					who: investor.clone(),
				}
				.into()
		}));

		// Order should not have been updated since everything is collected
		assert!(!events_since_collect.iter().any(|e| {
			e.event
				== pallet_investments::Event::<DevelopmentRuntime>::InvestOrderUpdated {
					investment_id: default_investment_id(),
					submitted_at: 0,
					who: investor.clone(),
					amount: 0,
				}
				.into()
		}));

		// Order should have been fully collected
		assert!(events_since_collect.iter().any(|e| {
			e.event
				== pallet_investments::Event::<DevelopmentRuntime>::InvestOrdersCollected {
					investment_id: default_investment_id(),
					processed_orders: vec![0],
					who: investor.clone(),
					collection: InvestCollection::<Balance> {
						payout_investment_invest: amount,
						remaining_investment_invest: 0,
					},
					outcome: CollectOutcome::FullyCollected,
				}
				.into()
		}));

		// Foreign InvestmentState should be killed
		assert!(!pallet_foreign_investments::InvestmentState::<
			DevelopmentRuntime,
		>::contains_key(investor.clone(), default_investment_id()));

		// Clearing of foreign InvestState should be dispatched
		assert!(events_since_collect.iter().any(|e| {
			e.event
				== pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignInvestmentCleared {
					investor: investor.clone(),
					investment_id: default_investment_id(),
				}
				.into()
		}));
	});
}

#[test]
fn inbound_increase_redeem_order() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();
		let pool_id = DEFAULT_POOL_ID;
		let amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;

		// Create new pool
		create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial redemption
		do_initial_increase_redemption(pool_id, amount, investor, currency_id);

		// Verify amount was noted in the corresponding order
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_redeem_order(
				default_investment_id(),
			)
			.amount,
			amount
		);

		// increase again, state should be SwapIntoForeignDone
	});
}

#[test]
fn inbound_decrease_redeem_order() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();
		let pool_id = DEFAULT_POOL_ID;
		let redeem_amount = 100_000_000;
		let decrease_amount = redeem_amount / 3;
		let final_amount = redeem_amount - decrease_amount;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;
		let sending_domain_locator = Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());

		// Create new pool
		create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial redemption
		do_initial_increase_redemption(pool_id, redeem_amount, investor.clone(), currency_id);

		// Verify the corresponding redemption order id is 0
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::invest_order_id(investment_id(
				pool_id,
				default_tranche_id(pool_id)
			)),
			0
		);

		// Mock incoming decrease message
		let msg = LiquidityPoolMessage::DecreaseRedeemOrder {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
			amount: decrease_amount,
		};

		// Execute byte message
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Verify investment was decreased into investment account
		assert_eq!(
			OrmlTokens::free_balance(
				default_investment_id().into(),
				&default_investment_account(),
			),
			final_amount
		);
		// Tokens should have been transferred from investor's wallet to domain's
		// sovereign account
		assert_eq!(
			OrmlTokens::free_balance(default_investment_id().into(), &investor),
			0
		);
		assert_eq!(
			OrmlTokens::free_balance(default_investment_id().into(), &sending_domain_locator),
			decrease_amount
		);

		// Foreign RedemptionState should be updated
		assert!(System::events().iter().any(|e| {
			e.event
				== pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignRedemptionUpdated {
					investor: investor.clone(),
					investment_id: default_investment_id(),
					state: RedeemState::InvestedAnd {
						invest_amount: decrease_amount,
						inner: InnerRedeemState::Redeeming {
							redeem_amount: final_amount
						}
					}
				}
			.into()
		}));

		// Order should have been updated
		assert!(System::events().iter().any(|e| e.event
			== pallet_investments::Event::<DevelopmentRuntime>::RedeemOrderUpdated {
				investment_id: default_investment_id(),
				submitted_at: 0,
				who: investor.clone(),
				amount: final_amount
			}
			.into()));
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_redeem_order(
				default_investment_id(),
			)
			.amount,
			final_amount
		);
	});
}

#[test]
fn inbound_cancel_redeem_order() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();
		let pool_id = DEFAULT_POOL_ID;
		let redeem_amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;
		let sending_domain_locator = Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());

		// Create new pool
		create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial redemption
		do_initial_increase_redemption(pool_id, redeem_amount, investor.clone(), currency_id);

		// Verify the corresponding redemption order id is 0
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::invest_order_id(investment_id(
				pool_id,
				default_tranche_id(pool_id)
			)),
			0
		);

		// Mock incoming decrease message
		let msg = LiquidityPoolMessage::CancelRedeemOrder {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
		};

		// Execute byte message
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Verify investment was decreased into investment account
		assert_eq!(
			OrmlTokens::free_balance(
				default_investment_id().into(),
				&default_investment_account(),
			),
			0
		);
		// Tokens should have been transferred from investor's wallet to domain's
		// sovereign account
		assert_eq!(
			OrmlTokens::free_balance(default_investment_id().into(), &investor),
			0
		);
		assert_eq!(
			OrmlTokens::free_balance(default_investment_id().into(), &sending_domain_locator),
			redeem_amount
		);

		// Foreign RedemptionState should be updated
		assert!(System::events().iter().any(|e| {
			e.event
				== pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignRedemptionUpdated {
					investor: investor.clone(),
					investment_id: default_investment_id(),
					state: RedeemState::Invested { invest_amount: redeem_amount },
				}
				.into()
		}));

		// Order should have been updated
		assert!(System::events().iter().any(|e| e.event
			== pallet_investments::Event::<DevelopmentRuntime>::RedeemOrderUpdated {
				investment_id: default_investment_id(),
				submitted_at: 0,
				who: investor.clone(),
				amount: 0
			}
			.into()));
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_redeem_order(
				default_investment_id(),
			)
			.amount,
			0
		);
	});
}

#[test]
fn inbound_collect_redeem_order() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();
		let pool_id = DEFAULT_POOL_ID;
		let amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;
		let pool_account =
			pallet_pool_system::pool_types::PoolLocator { pool_id }.into_account_truncating();

		// Create new pool
		create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial investment
		do_initial_increase_redemption(pool_id, amount, investor.clone(), currency_id);
		let events_before_collect = System::events();

		// Fund the pool account with sufficient pool currency, else redemption cannot
		// swap tranche tokens against pool currency
		assert_ok!(OrmlTokens::mint_into(currency_id, &pool_account, amount));

		// Process and fulfill order
		// NOTE: Without this step, the order id is not cleared and
		// `Event::RedeemCollectedForNonClearedOrderId` be dispatched
		assert_ok!(Investments::process_redeem_orders(default_investment_id()));
		assert_ok!(Investments::redeem_fulfillment(
			default_investment_id(),
			FulfillmentWithPrice::<Rate> {
				of_amount: Perquintill::one(),
				price: Rate::one(),
			}
		));

		// Enable liquidity pool transferability
		enable_liquidity_pool_transferability(currency_id);

		// Mock collection message msg
		let msg = LiquidityPoolMessage::CollectRedeem {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
		};

		// Execute byte message
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Remove events before collect execution
		let events_since_collect: Vec<_> = System::events()
			.into_iter()
			.filter(|e| !events_before_collect.contains(e))
			.collect();

		// Verify collected redemption was burned from investor
		assert_eq!(OrmlTokens::free_balance(currency_id, &investor), 0);
		assert!(System::events().iter().any(|e| e.event
			== orml_tokens::Event::<DevelopmentRuntime>::Withdrawn {
				currency_id,
				who: investor.clone(),
				amount
			}
			.into()));

		// Order should have been cleared by fulfilling redemption
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_redeem_order(
				default_investment_id(),
			)
			.amount,
			0
		);
		assert!(!events_since_collect.iter().any(|e| {
			e.event
				== pallet_investments::Event::<DevelopmentRuntime>::RedeemCollectedForNonClearedOrderId {
					investment_id: default_investment_id(),
					who: investor.clone(),
				}
				.into()
		}));

		// Order should not have been updated since everything is collected
		assert!(!events_since_collect.iter().any(|e| {
			e.event
				== pallet_investments::Event::<DevelopmentRuntime>::RedeemOrderUpdated {
					investment_id: default_investment_id(),
					submitted_at: 0,
					who: investor.clone(),
					amount: 0,
				}
				.into()
		}));

		// Order should have been fully collected
		assert!(events_since_collect.iter().any(|e| {
			e.event
				== pallet_investments::Event::<DevelopmentRuntime>::RedeemOrdersCollected {
					investment_id: default_investment_id(),
					processed_orders: vec![0],
					who: investor.clone(),
					collection: RedeemCollection::<Balance> {
						payout_investment_redeem: amount,
						remaining_investment_redeem: 0,
					},
					outcome: CollectOutcome::FullyCollected,
				}
				.into()
		}));

		// Foreign CollectedRedemptionTrancheTokens should be killed
		assert!(!pallet_foreign_investments::CollectedRedemptionTrancheTokens::<DevelopmentRuntime>::contains_key(
				investor.clone(),
				default_investment_id(),
			));

		// Foreign RedemptionState should be killed
		assert!(!pallet_foreign_investments::RedemptionState::<
			DevelopmentRuntime,
		>::contains_key(investor.clone(), default_investment_id()));

		// Clearing of foreign RedeemState should be dispatched
		assert!(events_since_collect.iter().any(|e| {
			e.event
					== pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignRedemptionCleared {
						investor: investor.clone(),
						investment_id: default_investment_id(),
					}
					.into()
		}));
	});
}

mod should_fail {
	use pallet_foreign_investments::errors::{InvestError, RedeemError};

	use super::*;

	mod decrease_should_underflow {
		use super::*;

		#[test]
		fn invest_decrease_underflow() {
			TestNet::reset();
			Development::execute_with(|| {
				setup_pre_requirements();
				let pool_id = DEFAULT_POOL_ID;
				let invest_amount: u128 = 100_000_000;
				let decrease_amount = invest_amount + 1;
				let investor: AccountId = BOB.into();
				let currency_id: CurrencyId = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				create_currency_pool(pool_id, currency_id, currency_decimals.into());
				do_initial_increase_investment(
					pool_id,
					invest_amount,
					investor.clone(),
					currency_id,
				);
				enable_liquidity_pool_transferability(currency_id);

				// Mock incoming decrease message
				let msg = LiquidityPoolMessage::DecreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index(currency_id),
					amount: decrease_amount,
				};

				assert_noop!(
					LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg),
					pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestError(
						InvestError::Decrease
					)
				);
			});
		}

		#[test]
		fn redeem_decrease_underflow() {
			TestNet::reset();
			Development::execute_with(|| {
				setup_pre_requirements();
				let pool_id = DEFAULT_POOL_ID;
				let redeem_amount: u128 = 100_000_000;
				let decrease_amount = redeem_amount + 1;
				let investor: AccountId = BOB.into();
				let currency_id: CurrencyId = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				create_currency_pool(pool_id, currency_id, currency_decimals.into());
				do_initial_increase_redemption(
					pool_id,
					redeem_amount,
					investor.clone(),
					currency_id,
				);

				// Mock incoming decrease message
				let msg = LiquidityPoolMessage::DecreaseRedeemOrder {
					pool_id,
					tranche_id: default_tranche_id(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index(currency_id),
					amount: decrease_amount,
				};

				assert_noop!(
					LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg),
					pallet_foreign_investments::Error::<DevelopmentRuntime>::RedeemError(
						RedeemError::Decrease
					)
				);
			});
		}
	}

	mod should_throw_requires_collect {
		use super::*;
		#[test]
		fn invest_requires_collect() {
			TestNet::reset();
			Development::execute_with(|| {
				setup_pre_requirements();
				let pool_id = DEFAULT_POOL_ID;
				let amount: u128 = 100_000_000;
				let investor: AccountId = BOB.into();
				let currency_id: CurrencyId = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				create_currency_pool(pool_id, currency_id, currency_decimals.into());
				do_initial_increase_investment(pool_id, amount, investor.clone(), currency_id);
				enable_liquidity_pool_transferability(currency_id);

				// Prepare collection
				let pool_account = pallet_pool_system::pool_types::PoolLocator { pool_id }
					.into_account_truncating();
				assert_ok!(OrmlTokens::mint_into(currency_id, &pool_account, amount));
				assert_ok!(Investments::process_invest_orders(default_investment_id()));
				assert_ok!(Investments::invest_fulfillment(
					default_investment_id(),
					FulfillmentWithPrice::<Rate> {
						of_amount: Perquintill::one(),
						price: Rate::one(),
					}
				));

				// Should fail to increase
				let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index(currency_id),
					amount: 1,
				};
				assert_noop!(
					LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, increase_msg),
					pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestError(
						InvestError::CollectRequired
					)
				);

				// Should fail to decrease
				let decrease_msg = LiquidityPoolMessage::DecreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index(currency_id),
					amount: 1,
				};
				assert_noop!(
					LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, decrease_msg),
					pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestError(
						InvestError::CollectRequired
					)
				);
			});
		}

		#[test]
		fn redeem_requires_collect() {
			TestNet::reset();
			Development::execute_with(|| {
				setup_pre_requirements();
				let pool_id = DEFAULT_POOL_ID;
				let amount: u128 = 100_000_000;
				let investor: AccountId = BOB.into();
				let currency_id: CurrencyId = AUSD_CURRENCY_ID;
				let currency_decimals = currency_decimals::AUSD;
				create_currency_pool(pool_id, currency_id, currency_decimals.into());
				do_initial_increase_redemption(pool_id, amount, investor.clone(), currency_id);
				enable_liquidity_pool_transferability(currency_id);

				// Mint more into DomainLocator required for subsequent invest attempt
				assert_ok!(OrmlTokens::mint_into(
					default_investment_id().into(),
					&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
					1,
				));

				// Prepare collection
				let pool_account = pallet_pool_system::pool_types::PoolLocator { pool_id }
					.into_account_truncating();
				assert_ok!(OrmlTokens::mint_into(currency_id, &pool_account, amount));
				assert_ok!(Investments::process_redeem_orders(default_investment_id()));
				assert_ok!(Investments::redeem_fulfillment(
					default_investment_id(),
					FulfillmentWithPrice::<Rate> {
						of_amount: Perquintill::one(),
						price: Rate::one(),
					}
				));

				// Should fail to increase
				let increase_msg = LiquidityPoolMessage::IncreaseRedeemOrder {
					pool_id,
					tranche_id: default_tranche_id(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index(currency_id),
					amount: 1,
				};
				assert_noop!(
					LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, increase_msg),
					pallet_foreign_investments::Error::<DevelopmentRuntime>::RedeemError(
						RedeemError::CollectRequired
					)
				);

				// Should fail to decrease
				let decrease_msg = LiquidityPoolMessage::DecreaseRedeemOrder {
					pool_id,
					tranche_id: default_tranche_id(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index(currency_id),
					amount: 1,
				};
				assert_noop!(
					LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, decrease_msg),
					pallet_foreign_investments::Error::<DevelopmentRuntime>::RedeemError(
						RedeemError::CollectRequired
					)
				);
			});
		}
	}

	mod mismatching_currencies {
		use super::*;
		use crate::utils::GLMR_CURRENCY_ID;

		#[test]
		fn invest_increase_another_currency() {
			TestNet::reset();
			Development::execute_with(|| {
				setup_pre_requirements();
				let pool_id = DEFAULT_POOL_ID;
				let invest_amount: u128 = 100_000_000;
				let investor: AccountId = BOB.into();
				let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
				let foreign_currency: CurrencyId = GLMR_CURRENCY_ID;
				let pool_currency_decimals = currency_decimals::AUSD;
				create_currency_pool(pool_id, pool_currency, pool_currency_decimals.into());
				do_initial_increase_investment(
					pool_id,
					invest_amount,
					investor.clone(),
					pool_currency,
				);
				enable_liquidity_pool_transferability(pool_currency);

				// Should fail to increase in another currency
				let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index(foreign_currency),
					amount: 1,
				};
				assert_noop!(
					LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, increase_msg),
					pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidPaymentCurrency
				);

				// TODO: Add foreign currency to accepted payment

				assert_noop!(
					LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, increase_msg),
					pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestError(
						InvestError::Increase
					)
				);

				// Should fail to decrease in another currency
				let decrease_msg = LiquidityPoolMessage::DecreaseInvestOrder {
					pool_id,
					tranche_id: default_tranche_id(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index(foreign_currency),
					amount: 1,
				};
				assert_noop!(
					LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, decrease_msg),
					pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestError(
						InvestError::Decrease
					)
				);
			});
		}

		// TODO: Similar tests for decreasing investments, increase/decrease and
		// collect redemption
	}
}

mod setup {
	use super::*;
	use crate::liquidity_pools::pallet::development::tests::liquidity_pools::setup::DEFAULT_OTHER_DOMAIN_ADDRESS;

	/// Sets up required permissions for the investor and executes an
	/// initial investment via LiquidityPools by executing
	/// `IncreaseInvestOrder`.
	///
	/// Assumes `setup_pre_requirements` and
	/// `investments::create_currency_pool` to have been called
	/// beforehand
	pub fn do_initial_increase_investment(
		pool_id: u64,
		amount: Balance,
		investor: AccountId,
		currency_id: CurrencyId,
	) {
		let valid_until = DEFAULT_VALIDITY;

		// Mock incoming increase invest message
		let msg = LiquidityPoolMessage::IncreaseInvestOrder {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
			amount,
		};

		// Should fail if investor does not have investor role yet
		assert_noop!(
			LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
			DispatchError::Other("Account does not have the TrancheInvestor permission.")
		);

		// Make investor the MembersListAdmin of this Pool
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			investor.clone(),
			PermissionScope::Pool(pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(
				default_tranche_id(pool_id),
				valid_until
			)),
		));

		let amount_before = OrmlTokens::free_balance(currency_id, &default_investment_account());
		let final_amount = amount_before
			.ensure_add(amount)
			.expect("Should not overflow when incrementing amount");

		// Execute byte message
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Verify investment was transferred into investment account
		assert_eq!(
			OrmlTokens::free_balance(currency_id, &default_investment_account()),
			final_amount
		);
		assert!(System::events().iter().any(|e| {
			e.event == pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignInvestmentUpdated {
                investor: investor.clone(),
                investment_id: default_investment_id(),
                state: InvestState::InvestmentOngoing {
                    invest_amount: final_amount
                },
            }
            .into()
		}));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			pallet_investments::Event::<DevelopmentRuntime>::InvestOrderUpdated {
				investment_id: default_investment_id(),
				submitted_at: 0,
				who: investor,
				amount: final_amount
			}
			.into()
		);
	}

	/// Sets up required permissions for the investor and executes an
	/// initial redemption via LiquidityPools by executing
	/// `IncreaseRedeemOrder`.
	///
	/// Assumes `setup_pre_requirements` and
	/// `investments::create_currency_pool` to have been called
	/// beforehand.
	///
	/// NOTE: Mints exactly the redeeming amount of tranche tokens.
	pub fn do_initial_increase_redemption(
		pool_id: u64,
		amount: Balance,
		investor: AccountId,
		currency_id: CurrencyId,
	) {
		let valid_until = DEFAULT_VALIDITY;

		// Fund `DomainLocator` account of origination domain as redeemed tranche tokens
		// are transferred from this account instead of minting
		assert_ok!(OrmlTokens::mint_into(
			default_investment_id().into(),
			&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
			amount
		));

		// Verify redemption has not been made yet
		assert_eq!(
			OrmlTokens::free_balance(
				default_investment_id().into(),
				&default_investment_account(),
			),
			0
		);
		assert_eq!(
			OrmlTokens::free_balance(default_investment_id().into(), &investor),
			0
		);

		// Mock incoming increase invest message
		let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
			pool_id: 42,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
			amount,
		};

		// Should fail if investor does not have investor role yet
		assert_noop!(
			LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
			DispatchError::Other("Account does not have the TrancheInvestor permission.")
		);

		// Make investor the MembersListAdmin of this Pool
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			investor.clone(),
			PermissionScope::Pool(pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(
				default_tranche_id(pool_id),
				valid_until
			)),
		));

		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Verify redemption was transferred into investment account
		assert_eq!(
			OrmlTokens::free_balance(
				default_investment_id().into(),
				&default_investment_account(),
			),
			amount
		);
		assert_eq!(
			OrmlTokens::free_balance(default_investment_id().into(), &investor),
			0
		);
		assert_eq!(
			OrmlTokens::free_balance(
				default_investment_id().into(),
				&AccountConverter::<DevelopmentRuntime>::convert(DEFAULT_OTHER_DOMAIN_ADDRESS)
			),
			0
		);
		assert_eq!(
			System::events().iter().nth_back(4).unwrap().event,
			pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignRedemptionUpdated {
				investor: investor.clone(),
				investment_id: default_investment_id(),
				state: RedeemState::NotInvestedAnd {
					inner: InnerRedeemState::Redeeming {
						redeem_amount: amount
					}
				},
			}
			.into()
		);
		assert_eq!(
			System::events().iter().last().unwrap().event,
			pallet_investments::Event::<DevelopmentRuntime>::RedeemOrderUpdated {
				investment_id: default_investment_id(),
				submitted_at: 0,
				who: investor,
				amount
			}
			.into()
		);

		// Verify order id is 0
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::redeem_order_id(investment_id(
				pool_id,
				default_tranche_id(pool_id)
			)),
			0
		);
	}
}
