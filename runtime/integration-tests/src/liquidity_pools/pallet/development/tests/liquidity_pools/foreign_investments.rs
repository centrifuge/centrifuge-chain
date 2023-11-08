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
	investments::{Investment, OrderManager, TrancheCurrency as TrancheCurrencyT},
	liquidity_pools::InboundQueue,
	IdentityCurrencyConversion, PoolInspect,
};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Ratio,
	investments::{CollectedAmount, InvestCollection, InvestmentAccount, RedeemCollection, Swap},
	orders::FulfillmentWithPrice,
	permissions::{PermissionScope, PoolRole, Role, UNION},
	pools::TrancheMetadata,
	tokens::{
		CrossChainTransferability, CurrencyId, CurrencyId::ForeignAsset, CustomMetadata,
		ForeignAssetId, TrancheCurrency,
	},
};
use frame_support::{
	assert_noop, assert_ok,
	traits::{
		fungibles::{Inspect, Mutate},
		Get, PalletInfo,
	},
};
use fudge::primitives::Chain;
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use pallet_foreign_investments::{
	types::{InvestState, RedeemState},
	CollectedInvestment, CollectedRedemption, InvestmentPaymentCurrency, InvestmentState,
	RedemptionPayoutCurrency, RedemptionState,
};
use pallet_investments::CollectOutcome;
use runtime_common::{
	account_conversion::AccountConverter, foreign_investments::IdentityPoolCurrencyConverter,
};
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, ConstU32, Convert, EnsureAdd, One, Zero},
	BoundedVec, DispatchError, FixedPointNumber, Perquintill, SaturatedConversion, Storage,
	WeakBoundedVec,
};
use tokio::runtime::Handle;

use crate::{
	chain::centrifuge::{
		Balances, ForeignInvestments, Investments, LiquidityPools, LocationToAccountId,
		MinFulfillmentAmountNative, OrmlAssetRegistry, Permissions, PoolSystem,
		Runtime as DevelopmentRuntime, RuntimeOrigin, System, Tokens, TreasuryAccount, PARA_ID,
	},
	liquidity_pools::pallet::development::{
		setup::dollar,
		tests::liquidity_pools::{
			foreign_investments::setup::{
				do_initial_increase_investment, do_initial_increase_redemption,
				ensure_executed_collect_redeem_not_dispatched, min_fulfillment_amount,
			},
			setup::{
				asset_metadata, create_ausd_pool, create_currency_pool,
				enable_liquidity_pool_transferability,
				investments::{
					default_investment_account, default_investment_id, default_tranche_id,
					general_currency_index, investment_id,
				},
				setup_test_env, LiquidityPoolMessage, DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				DEFAULT_POOL_ID, DEFAULT_VALIDITY, DOMAIN_MOONBEAM,
			},
		},
	},
	utils::{accounts::Keyring, env, genesis, AUSD_CURRENCY_ID, AUSD_ED},
};

mod same_currencies {
	use pallet_foreign_investments::errors::InvestError;

	use super::*;

	#[tokio::test]
	async fn increase_invest_order() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let amount = 10 * dollar(12);
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let currency_id = AUSD_CURRENCY_ID;
			let currency_decimals = currency_decimals::AUSD;

			// Create new pool
			create_currency_pool(pool_id, currency_id, currency_decimals.into());

			// Set permissions and execute initial investment
			do_initial_increase_investment(pool_id, amount, investor.clone(), currency_id, false);

			// Verify the order was updated to the amount
			assert_eq!(
				pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_invest_order(
					default_investment_id(),
				)
				.amount,
				amount
			);

			// Increasing again should just bump invest_amount
			let msg = LiquidityPoolMessage::IncreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(currency_id),
				amount,
			};
			assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing {
					invest_amount: amount * 2
				}
			);
		});
	}

	#[tokio::test]
	async fn decrease_invest_order() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let invest_amount: u128 = 10 * dollar(12);
			let decrease_amount = invest_amount / 3;
			let final_amount = invest_amount - decrease_amount;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let currency_id: CurrencyId = AUSD_CURRENCY_ID;
			let currency_decimals = currency_decimals::AUSD;

			// Create new pool
			create_currency_pool(pool_id, currency_id, currency_decimals.into());

			// Set permissions and execute initial investment
			do_initial_increase_investment(
				pool_id,
				invest_amount,
				investor.clone(),
				currency_id,
				false,
			);

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
				Tokens::balance(currency_id, &default_investment_account()),
				final_amount
			);
			// Since the investment was done in the pool currency, the decrement happens
			// synchronously and thus it must be burned from investor's holdings
			assert_eq!(Tokens::balance(currency_id, &investor), 0);
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

	#[tokio::test]
	async fn cancel_invest_order() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let invest_amount = 10 * dollar(12);
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let currency_id = AUSD_CURRENCY_ID;
			let currency_decimals = currency_decimals::AUSD;

			// Create new pool
			create_currency_pool(pool_id, currency_id, currency_decimals.into());

			// Set permissions and execute initial investment
			do_initial_increase_investment(
				pool_id,
				invest_amount,
				investor.clone(),
				currency_id,
				false,
			);

			// Verify investment account holds funds before cancelling
			assert_eq!(
				Tokens::balance(currency_id, &default_investment_account()),
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
				Tokens::balance(currency_id, &default_investment_account()),
				0
			);
			// Since the investment was done in the pool currency, the decrement happens
			// synchronously and thus it must be burned from investor's holdings
			assert_eq!(Tokens::balance(currency_id, &investor), 0);
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

	#[tokio::test]
	async fn collect_invest_order() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let amount = 10 * dollar(12);
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let currency_id = AUSD_CURRENCY_ID;
			let currency_decimals = currency_decimals::AUSD;
			let sending_domain_locator = Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
			enable_liquidity_pool_transferability(currency_id);

			// Create new pool
			create_currency_pool(pool_id, currency_id, currency_decimals.into());
			let investment_currency_id: CurrencyId = default_investment_id().into();

			// Set permissions and execute initial investment
			do_initial_increase_investment(pool_id, amount, investor.clone(), currency_id, false);
			let events_before_collect = System::events();

			// Process and fulfill order
			// NOTE: Without this step, the order id is not cleared and
			// `Event::InvestCollectedForNonClearedOrderId` be dispatched
			assert_ok!(Investments::process_invest_orders(default_investment_id()));

			// Tranche tokens will be minted upon fulfillment
			assert_eq!(Tokens::total_issuance(investment_currency_id), 0);
			assert_ok!(Investments::invest_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::one(),
					price: Ratio::one(),
				}
			));
			assert_eq!(Tokens::total_issuance(investment_currency_id), amount);

			// Mock collection message msg
			let msg = LiquidityPoolMessage::CollectInvest {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(currency_id),
			};
			assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

			// Remove events before collect execution
			let events_since_collect: Vec<_> = System::events()
				.into_iter()
				.filter(|e| !events_before_collect.contains(e))
				.collect();

			// Verify investment was transferred to the domain locator
			assert_eq!(
				Tokens::balance(default_investment_id().into(), &sending_domain_locator),
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

			assert!(!CollectedInvestment::<DevelopmentRuntime>::contains_key(
				investor.clone(),
				default_investment_id()
			));
			assert!(
				!InvestmentPaymentCurrency::<DevelopmentRuntime>::contains_key(
					investor.clone(),
					default_investment_id()
				)
			);
			assert!(!InvestmentState::<DevelopmentRuntime>::contains_key(
				investor.clone(),
				default_investment_id()
			));

			// Clearing of foreign InvestState should be dispatched
			assert!(events_since_collect.iter().any(|e| {
				e.event
					== pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignInvestmentCleared {
					investor: investor.clone(),
					investment_id: default_investment_id(),
				}
					.into()
			}));

			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectInvest {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(currency_id),
							currency_payout: amount,
							tranche_tokens_payout: amount,
							remaining_invest_amount: 0,
						},
					}
					.into()
			}));
		});
	}

	#[tokio::test]
	async fn partially_collect_investment_for_through_investments() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let invest_amount = 10 * dollar(12);
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let currency_id = AUSD_CURRENCY_ID;
			let currency_decimals = currency_decimals::AUSD;
			let sending_domain_locator = Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
			create_currency_pool(pool_id, currency_id, currency_decimals.into());
			do_initial_increase_investment(
				pool_id,
				invest_amount,
				investor.clone(),
				currency_id,
				false,
			);
			enable_liquidity_pool_transferability(currency_id);
			let investment_currency_id: CurrencyId = default_investment_id().into();

			assert!(!Investments::investment_requires_collect(
				&investor,
				default_investment_id()
			));

			// Process 50% of investment at 25% rate, i.e. 1 pool currency = 4 tranche
			// tokens
			assert_ok!(Investments::process_invest_orders(default_investment_id()));
			assert_ok!(Investments::invest_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(50),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));

			// Pre collect assertions
			assert!(Investments::investment_requires_collect(
				&investor,
				default_investment_id()
			));
			assert!(!CollectedInvestment::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing { invest_amount }
			);

			// Collecting through Investments should denote amounts and transition
			// state
			assert_ok!(Investments::collect_investments_for(
				RuntimeOrigin::signed(Keyring::Alice.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				InvestmentPaymentCurrency::<DevelopmentRuntime>::get(
					&investor,
					default_investment_id()
				)
				.unwrap(),
				currency_id
			);
			assert!(!Investments::investment_requires_collect(
				&investor,
				default_investment_id()
			));
			// The collected amount is transferred automatically
			assert!(!CollectedInvestment::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing {
					invest_amount: invest_amount / 2
				}
			);
			// Tranche Tokens should still be transferred to collected to
			// domain locator account already
			assert_eq!(Tokens::balance(investment_currency_id, &investor), 0);
			assert_eq!(
				Tokens::balance(investment_currency_id, &sending_domain_locator),
				invest_amount * 2
			);
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_investments::Event::<DevelopmentRuntime>::InvestOrdersCollected {
						investment_id: default_investment_id(),
						processed_orders: vec![0],
						who: investor.clone(),
						collection: InvestCollection::<Balance> {
							payout_investment_invest: invest_amount * 2,
							remaining_investment_invest: invest_amount / 2,
						},
						outcome: CollectOutcome::FullyCollected,
					}
					.into()
			}));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectInvest {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(currency_id),
							currency_payout: invest_amount / 2,
							tranche_tokens_payout: invest_amount * 2,
							remaining_invest_amount: invest_amount / 2,
						},
					}
					.into()
			}));

			// Process rest of investment at 50% rate (1 pool currency = 2 tranche tokens)
			assert_ok!(Investments::process_invest_orders(default_investment_id()));
			assert_ok!(Investments::invest_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::one(),
					price: Ratio::checked_from_rational(1, 2).unwrap(),
				}
			));
			// Order should have been cleared by fulfilling investment
			assert_eq!(
				pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_invest_order(
					default_investment_id(),
				)
				.amount,
				0
			);
			assert_eq!(
				Tokens::total_issuance(investment_currency_id),
				invest_amount * 3
			);

			// Collect remainder through Investments
			assert_ok!(Investments::collect_investments_for(
				RuntimeOrigin::signed(Keyring::Alice.into()),
				investor.clone(),
				default_investment_id()
			));
			assert!(!Investments::investment_requires_collect(
				&investor,
				default_investment_id()
			));
			assert!(!CollectedInvestment::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			));
			assert!(
				!InvestmentPaymentCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				),
			);
			assert!(
				!InvestmentPaymentCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				),
			);
			assert!(!InvestmentState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			));
			// Tranche Tokens should be transferred to collected to
			// domain locator account already
			let amount_tranche_tokens = invest_amount * 3;
			assert_eq!(
				Tokens::total_issuance(investment_currency_id),
				amount_tranche_tokens
			);
			assert!(Tokens::balance(investment_currency_id, &investor).is_zero());
			assert_eq!(
				Tokens::balance(investment_currency_id, &sending_domain_locator),
				amount_tranche_tokens
			);
			assert!(!System::events().iter().any(|e| {
				e.event
					== pallet_investments::Event::<DevelopmentRuntime>::InvestCollectedForNonClearedOrderId {
					investment_id: default_investment_id(),
					who: investor.clone(),
				}
					.into()
			}));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_investments::Event::<DevelopmentRuntime>::InvestOrdersCollected {
						investment_id: default_investment_id(),
						processed_orders: vec![1],
						who: investor.clone(),
						collection: InvestCollection::<Balance> {
							payout_investment_invest: invest_amount,
							remaining_investment_invest: 0,
						},
						outcome: CollectOutcome::FullyCollected,
					}
					.into()
			}));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectInvest {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(currency_id),
							currency_payout: invest_amount / 2,
							tranche_tokens_payout: invest_amount,
							remaining_invest_amount: 0,
						},
					}
					.into()
			}));
			// Clearing of foreign InvestState should have been dispatched exactly once
			assert_eq!(
				System::events()
					.iter()
					.filter(|e| {
						e.event
							== pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignInvestmentCleared {
							investor: investor.clone(),
							investment_id: default_investment_id(),
						}
							.into()
					})
					.count(),
				1
			);

			// Should fail to collect if `InvestmentState` does not exist
			let msg = LiquidityPoolMessage::CollectInvest {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(currency_id),
			};
			assert_noop!(
				LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg),
				pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestmentPaymentCurrencyNotFound
			);
		});
	}

	#[tokio::test]
	async fn increase_redeem_order() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let amount = 10 * dollar(12);
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let currency_id = AUSD_CURRENCY_ID;
			let currency_decimals = currency_decimals::AUSD;

			// Create new pool
			create_currency_pool(pool_id, currency_id, currency_decimals.into());

			// Set permissions and execute initial redemption
			do_initial_increase_redemption(pool_id, amount, investor.clone(), currency_id);

			// Verify amount was noted in the corresponding order
			assert_eq!(
				pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_redeem_order(
					default_investment_id(),
				)
				.amount,
				amount
			);

			// Increasing again should just bump redeeming amount
			assert_ok!(Tokens::mint_into(
				default_investment_id().into(),
				&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
				amount
			));
			let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(currency_id),
				amount,
			};
			assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::Redeeming {
					redeem_amount: amount * 2,
				}
			);
		});
	}

	#[tokio::test]
	async fn decrease_redeem_order() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let redeem_amount = 10 * dollar(12);
			let decrease_amount = redeem_amount / 3;
			let final_amount = redeem_amount - decrease_amount;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
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
				Tokens::balance(
					default_investment_id().into(),
					&default_investment_account(),
				),
				final_amount
			);
			// Tokens should have been transferred from investor's wallet to domain's
			// sovereign account
			assert_eq!(
				Tokens::balance(default_investment_id().into(), &investor),
				0
			);
			assert_eq!(
				Tokens::balance(default_investment_id().into(), &sending_domain_locator),
				decrease_amount
			);

			// Foreign RedemptionState should be updated
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignRedemptionUpdated {
					investor: investor.clone(),
					investment_id: default_investment_id(),
					state: RedeemState::Redeeming {
						redeem_amount: final_amount
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

			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedDecreaseRedeemOrder {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(currency_id),
							tranche_tokens_payout: decrease_amount,
							remaining_redeem_amount: final_amount,
						},
					}
					.into()
			}));
		});
	}

	#[tokio::test]
	async fn cancel_redeem_order() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let redeem_amount = 10 * dollar(12);
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
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
				Tokens::balance(
					default_investment_id().into(),
					&default_investment_account(),
				),
				0
			);
			// Tokens should have been transferred from investor's wallet to domain's
			// sovereign account
			assert_eq!(
				Tokens::balance(default_investment_id().into(), &investor),
				0
			);
			assert_eq!(
				Tokens::balance(default_investment_id().into(), &sending_domain_locator),
				redeem_amount
			);
			assert!(
				!RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);

			// Foreign RedemptionState should be updated
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignRedemptionCleared {
					investor: investor.clone(),
					investment_id: default_investment_id(),
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

	#[tokio::test]
	async fn fully_collect_redeem_order() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let amount = 10 * dollar(12);
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
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
			assert_ok!(Tokens::mint_into(currency_id, &pool_account, amount));

			// Process and fulfill order
			// NOTE: Without this step, the order id is not cleared and
			// `Event::RedeemCollectedForNonClearedOrderId` be dispatched
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::one(),
					price: Ratio::one(),
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
			assert_eq!(Tokens::balance(currency_id, &investor), 0);
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
			assert!(!pallet_foreign_investments::CollectedRedemption::<
				DevelopmentRuntime,
			>::contains_key(investor.clone(), default_investment_id(),));

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
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectRedeem {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(currency_id),
							currency_payout: amount,
							tranche_tokens_payout: amount,
							remaining_redeem_amount: 0,
						},
					}
					.into()
			}));
		});
	}

	#[tokio::test]
	async fn partially_collect_redemption_for_through_investments() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let redeem_amount = 10 * dollar(12);
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let currency_id = AUSD_CURRENCY_ID;
			let currency_decimals = currency_decimals::AUSD;
			let pool_account =
				pallet_pool_system::pool_types::PoolLocator { pool_id }.into_account_truncating();
			create_currency_pool(pool_id, currency_id, currency_decimals.into());
			do_initial_increase_redemption(pool_id, redeem_amount, investor.clone(), currency_id);
			enable_liquidity_pool_transferability(currency_id);

			// Fund the pool account with sufficient pool currency, else redemption cannot
			// swap tranche tokens against pool currency
			assert_ok!(Tokens::mint_into(currency_id, &pool_account, redeem_amount));
			assert!(!Investments::redemption_requires_collect(
				&investor,
				default_investment_id()
			));

			// Process 50% of redemption at 25% rate, i.e. 1 pool currency = 4 tranche
			// tokens
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(50),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));

			// Pre collect assertions
			assert!(Investments::redemption_requires_collect(
				&investor,
				default_investment_id()
			));
			assert!(!CollectedRedemption::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			));
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::Redeeming { redeem_amount }
			);
			// Collecting through investments should denote amounts and transition
			// state
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Alice.into()),
				investor.clone(),
				default_investment_id()
			));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_investments::Event::<DevelopmentRuntime>::RedeemOrdersCollected {
						investment_id: default_investment_id(),
						processed_orders: vec![0],
						who: investor.clone(),
						collection: RedeemCollection::<Balance> {
							payout_investment_redeem: redeem_amount / 8,
							remaining_investment_redeem: redeem_amount / 2,
						},
						outcome: CollectOutcome::FullyCollected,
					}
					.into()
			}));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectRedeem {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(currency_id),
							currency_payout: redeem_amount / 8,
							tranche_tokens_payout: redeem_amount / 2,
							remaining_redeem_amount: redeem_amount / 2,
						},
					}
					.into()
			}));
			assert!(!Investments::redemption_requires_collect(
				&investor,
				default_investment_id()
			));
			// Since foreign currency is pool currency, the swap is immediately fulfilled
			// and ExecutedCollectRedeem dispatched
			assert!(!CollectedRedemption::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::Redeeming {
					redeem_amount: redeem_amount / 2,
				}
			);
			assert!(System::events().iter().any(|e| e.event
				== orml_tokens::Event::<DevelopmentRuntime>::Withdrawn {
					currency_id,
					who: investor.clone(),
					amount: redeem_amount / 8
				}
				.into()));

			// Process rest of redemption at 50% rate
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::one(),
					price: Ratio::checked_from_rational(1, 2).unwrap(),
				}
			));
			// Order should have been cleared by fulfilling redemption
			assert_eq!(
				pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_redeem_order(
					default_investment_id(),
				)
				.amount,
				0
			);

			// Collect remainder through Investments
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Alice.into()),
				investor.clone(),
				default_investment_id()
			));
			assert!(!Investments::redemption_requires_collect(
				&investor,
				default_investment_id()
			));
			assert!(!CollectedRedemption::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			));
			assert!(!RedemptionState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			));
			assert!(!System::events().iter().any(|e| {
				e.event
					== pallet_investments::Event::<DevelopmentRuntime>::RedeemCollectedForNonClearedOrderId {
					investment_id: default_investment_id(),
					who: investor.clone(),
				}
					.into()
			}));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_investments::Event::<DevelopmentRuntime>::RedeemOrdersCollected {
						investment_id: default_investment_id(),
						processed_orders: vec![1],
						who: investor.clone(),
						collection: RedeemCollection::<Balance> {
							payout_investment_redeem: redeem_amount / 4,
							remaining_investment_redeem: 0,
						},
						outcome: CollectOutcome::FullyCollected,
					}
					.into()
			}));
			// Verify collected redemption was burned from investor
			assert_eq!(Tokens::balance(currency_id, &investor), 0);
			assert!(System::events().iter().any(|e| e.event
				== orml_tokens::Event::<DevelopmentRuntime>::Withdrawn {
					currency_id,
					who: investor.clone(),
					amount: redeem_amount / 4
				}
				.into()));
			// Clearing of foreign RedeemState should have been dispatched exactly once
			assert_eq!(
				System::events()
					.iter()
					.filter(|e| {
						e.event
							== pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignRedemptionCleared {
							investor: investor.clone(),
							investment_id: default_investment_id(),
						}
							.into()
					})
					.count(),
				1
			);
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectRedeem {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(currency_id),
							currency_payout: redeem_amount / 4,
							tranche_tokens_payout: redeem_amount / 2,
							remaining_redeem_amount: 0,
						},
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

			#[tokio::test]
			async fn invest_decrease_underflow() {
				let mut env = {
					let mut genesis = Storage::default();
					genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
					env::test_env_with_centrifuge_storage(Handle::current(), genesis)
				};

				setup_test_env(&mut env);

				env.with_mut_state(Chain::Para(PARA_ID), || {
					let pool_id = DEFAULT_POOL_ID;
					let invest_amount: u128 = 10 * dollar(12);
					let decrease_amount = invest_amount + 1;
					let investor: AccountId = AccountConverter::<
						DevelopmentRuntime,
						LocationToAccountId,
					>::convert((DOMAIN_MOONBEAM, Keyring::Bob.into()));
					let currency_id: CurrencyId = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;
					create_currency_pool(pool_id, currency_id, currency_decimals.into());
					do_initial_increase_investment(
						pool_id,
						invest_amount,
						investor.clone(),
						currency_id,
						false,
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
							InvestError::DecreaseAmountOverflow
						)
					);
				});
			}

			#[tokio::test]
			async fn redeem_decrease_underflow() {
				let mut env = {
					let mut genesis = Storage::default();
					genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
					env::test_env_with_centrifuge_storage(Handle::current(), genesis)
				};

				setup_test_env(&mut env);

				env.with_mut_state(Chain::Para(PARA_ID), || {
					let pool_id = DEFAULT_POOL_ID;
					let redeem_amount: u128 = 10 * dollar(12);
					let decrease_amount = redeem_amount + 1;
					let investor: AccountId = AccountConverter::<
						DevelopmentRuntime,
						LocationToAccountId,
					>::convert((DOMAIN_MOONBEAM, Keyring::Bob.into()));
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
							RedeemError::DecreaseTransition
						)
					);
				});
			}
		}

		mod should_throw_requires_collect {
			use super::*;

			#[tokio::test]
			async fn invest_requires_collect() {
				let mut env = {
					let mut genesis = Storage::default();
					genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
					env::test_env_with_centrifuge_storage(Handle::current(), genesis)
				};

				setup_test_env(&mut env);

				env.with_mut_state(Chain::Para(PARA_ID), || {
					let pool_id = DEFAULT_POOL_ID;
					let amount: u128 = 10 * dollar(12);
					let investor: AccountId = AccountConverter::<
						DevelopmentRuntime,
						LocationToAccountId,
					>::convert((DOMAIN_MOONBEAM, Keyring::Bob.into()));
					let currency_id: CurrencyId = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;
					create_currency_pool(pool_id, currency_id, currency_decimals.into());
					do_initial_increase_investment(
						pool_id,
						amount,
						investor.clone(),
						currency_id,
						false,
					);
					enable_liquidity_pool_transferability(currency_id);

					// Prepare collection
					let pool_account = pallet_pool_system::pool_types::PoolLocator { pool_id }
						.into_account_truncating();
					assert_ok!(Tokens::mint_into(currency_id, &pool_account, amount));
					assert_ok!(Investments::process_invest_orders(default_investment_id()));
					assert_ok!(Investments::invest_fulfillment(
						default_investment_id(),
						FulfillmentWithPrice {
							of_amount: Perquintill::one(),
							price: Ratio::one(),
						}
					));

					// Should fail to increase
					let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(currency_id),
						amount: AUSD_ED,
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

			#[tokio::test]
			async fn redeem_requires_collect() {
				let mut env = {
					let mut genesis = Storage::default();
					genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
					env::test_env_with_centrifuge_storage(Handle::current(), genesis)
				};

				setup_test_env(&mut env);

				env.with_mut_state(Chain::Para(PARA_ID), || {
					let pool_id = DEFAULT_POOL_ID;
					let amount: u128 = 10 * dollar(12);
					let investor: AccountId = AccountConverter::<
						DevelopmentRuntime,
						LocationToAccountId,
					>::convert((DOMAIN_MOONBEAM, Keyring::Bob.into()));
					let currency_id: CurrencyId = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;
					create_currency_pool(pool_id, currency_id, currency_decimals.into());
					do_initial_increase_redemption(pool_id, amount, investor.clone(), currency_id);
					enable_liquidity_pool_transferability(currency_id);

					// Mint more into DomainLocator required for subsequent invest attempt
					assert_ok!(Tokens::mint_into(
						default_investment_id().into(),
						&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
						1,
					));

					// Prepare collection
					let pool_account = pallet_pool_system::pool_types::PoolLocator { pool_id }
						.into_account_truncating();
					assert_ok!(Tokens::mint_into(currency_id, &pool_account, amount));
					assert_ok!(Investments::process_redeem_orders(default_investment_id()));
					assert_ok!(Investments::redeem_fulfillment(
						default_investment_id(),
						FulfillmentWithPrice {
							of_amount: Perquintill::one(),
							price: Ratio::one(),
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

		mod payment_payout_currency {
			use super::*;
			use crate::{
				liquidity_pools::pallet::development::tests::liquidity_pools::foreign_investments::setup::enable_usdt_trading,
				utils::USDT_CURRENCY_ID,
			};

			#[tokio::test]
			async fn invalid_invest_payment_currency() {
				let mut env = {
					let mut genesis = Storage::default();
					genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
					env::test_env_with_centrifuge_storage(Handle::current(), genesis)
				};

				setup_test_env(&mut env);

				env.with_mut_state(Chain::Para(PARA_ID), || {
					let pool_id = DEFAULT_POOL_ID;
					let investor: AccountId = AccountConverter::<
						DevelopmentRuntime,
						LocationToAccountId,
					>::convert((DOMAIN_MOONBEAM, Keyring::Bob.into()));
					let pool_currency = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;
					let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
					let amount = 6 * dollar(18);

					create_currency_pool(pool_id, pool_currency, currency_decimals.into());
					do_initial_increase_investment(
						pool_id,
						amount,
						investor.clone(),
						pool_currency,
						false,
					);
					enable_usdt_trading(pool_currency, amount, true, true, true, || {});

					// Should fail to increase, decrease or collect for another foreign currency as
					// long as `InvestmentState` exists
					let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
						amount: AUSD_ED,
					};
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, increase_msg),
						pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestError(
							InvestError::InvalidPaymentCurrency
						)
					);
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
							InvestError::InvalidPaymentCurrency
						)
					);
					let collect_msg = LiquidityPoolMessage::CollectInvest {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
					};
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, collect_msg),
						pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestError(
							InvestError::InvalidPaymentCurrency
						)
					);
				});
			}

			#[tokio::test]
			async fn invalid_redeem_payout_currency() {
				let mut env = {
					let mut genesis = Storage::default();
					genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
					env::test_env_with_centrifuge_storage(Handle::current(), genesis)
				};

				setup_test_env(&mut env);

				env.with_mut_state(Chain::Para(PARA_ID), || {
					let pool_id = DEFAULT_POOL_ID;
					let investor: AccountId = AccountConverter::<
						DevelopmentRuntime,
						LocationToAccountId,
					>::convert((DOMAIN_MOONBEAM, Keyring::Bob.into()));
					let pool_currency = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;
					let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
					let amount = 6 * dollar(18);

					create_currency_pool(pool_id, pool_currency, currency_decimals.into());
					do_initial_increase_redemption(
						pool_id,
						amount,
						investor.clone(),
						pool_currency,
					);
					enable_usdt_trading(pool_currency, amount, true, true, true, || {});
					assert_ok!(Tokens::mint_into(
						default_investment_id().into(),
						&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
						amount,
					));

					// Should fail to increase, decrease or collect for another foreign currency as
					// long as `RedemptionState` exists
					let increase_msg = LiquidityPoolMessage::IncreaseRedeemOrder {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
						amount: 1,
					};
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, increase_msg),
						pallet_foreign_investments::Error::<DevelopmentRuntime>::RedeemError(
							RedeemError::InvalidPayoutCurrency
						)
					);
					let decrease_msg = LiquidityPoolMessage::DecreaseRedeemOrder {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
						amount: 1,
					};
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, decrease_msg),
						pallet_foreign_investments::Error::<DevelopmentRuntime>::RedeemError(
							RedeemError::InvalidPayoutCurrency
						)
					);
					let collect_msg = LiquidityPoolMessage::CollectRedeem {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
					};
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, collect_msg),
						pallet_foreign_investments::Error::<DevelopmentRuntime>::RedeemError(
							RedeemError::InvalidPayoutCurrency
						)
					);
				});
			}

			#[tokio::test]
			async fn invest_payment_currency_not_found() {
				let mut env = {
					let mut genesis = Storage::default();
					genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
					env::test_env_with_centrifuge_storage(Handle::current(), genesis)
				};

				setup_test_env(&mut env);

				env.with_mut_state(Chain::Para(PARA_ID), || {
					let pool_id = DEFAULT_POOL_ID;
					let investor: AccountId = AccountConverter::<
						DevelopmentRuntime,
						LocationToAccountId,
					>::convert((DOMAIN_MOONBEAM, Keyring::Bob.into()));
					let pool_currency = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;
					let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
					let amount = 6 * dollar(18);

					create_currency_pool(pool_id, pool_currency, currency_decimals.into());
					do_initial_increase_investment(
						pool_id,
						amount,
						investor.clone(),
						pool_currency,
						true,
					);
					enable_usdt_trading(pool_currency, amount, true, true, true, || {});

					// Should fail to decrease or collect for another foreign currency as
					// long as `InvestmentState` exists
					let decrease_msg = LiquidityPoolMessage::DecreaseInvestOrder {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
						amount: 1,
					};
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, decrease_msg),
						pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestmentPaymentCurrencyNotFound
					);
					let collect_msg = LiquidityPoolMessage::CollectInvest {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
					};
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, collect_msg),
						pallet_foreign_investments::Error::<DevelopmentRuntime>::InvestmentPaymentCurrencyNotFound
					);
				});
			}

			#[tokio::test]
			async fn redeem_payout_currency_not_found() {
				let mut env = {
					let mut genesis = Storage::default();
					genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
					env::test_env_with_centrifuge_storage(Handle::current(), genesis)
				};

				setup_test_env(&mut env);

				env.with_mut_state(Chain::Para(PARA_ID), || {
					let pool_id = DEFAULT_POOL_ID;
					let investor: AccountId = AccountConverter::<
						DevelopmentRuntime,
						LocationToAccountId,
					>::convert((DOMAIN_MOONBEAM, Keyring::Bob.into()));
					let pool_currency = AUSD_CURRENCY_ID;
					let currency_decimals = currency_decimals::AUSD;
					let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
					let amount = 6 * dollar(18);

					create_currency_pool(pool_id, pool_currency, currency_decimals.into());
					do_initial_increase_redemption(
						pool_id,
						amount,
						investor.clone(),
						pool_currency,
					);
					enable_usdt_trading(pool_currency, amount, true, true, true, || {});
					assert_ok!(Tokens::mint_into(
						default_investment_id().into(),
						&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
						amount,
					));
					RedemptionPayoutCurrency::<DevelopmentRuntime>::remove(
						&investor,
						default_investment_id(),
					);

					// Should fail to decrease or collect for another foreign currency as
					// long as `RedemptionState` exists
					let decrease_msg = LiquidityPoolMessage::DecreaseRedeemOrder {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
						amount: 1,
					};
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, decrease_msg),
						pallet_foreign_investments::Error::<DevelopmentRuntime>::RedemptionPayoutCurrencyNotFound
					);
					let collect_msg = LiquidityPoolMessage::CollectRedeem {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
					};
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, collect_msg),
						pallet_foreign_investments::Error::<DevelopmentRuntime>::RedemptionPayoutCurrencyNotFound
					);
				});
			}
		}
	}
}

mod mismatching_currencies {
	use cfg_traits::investments::ForeignInvestment;
	use cfg_types::investments::{ForeignInvestmentInfo, Swap};
	use development_runtime::OrderBook;
	use pallet_foreign_investments::{types::TokenSwapReason, InvestmentState};

	use super::*;
	use crate::{
		liquidity_pools::pallet::development::tests::{
			liquidity_pools::foreign_investments::setup::enable_usdt_trading, register_usdt,
		},
		utils::{GLMR_CURRENCY_ID, USDT_CURRENCY_ID},
	};

	#[tokio::test]
	async fn collect_foreign_investment_for() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
			let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
			let pool_currency_decimals = currency_decimals::AUSD;
			let invest_amount_pool_denominated: u128 = 6 * dollar(18);
			let sending_domain_locator = Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
			create_currency_pool(pool_id, pool_currency, pool_currency_decimals.into());
			let invest_amount_foreign_denominated: u128 = enable_usdt_trading(
				pool_currency,
				invest_amount_pool_denominated,
				true,
				true,
				// not needed because we don't initialize a swap from pool to foreign here
				false,
				|| {},
			);

			do_initial_increase_investment(
				pool_id,
				invest_amount_pool_denominated,
				investor.clone(),
				pool_currency,
				true,
			);

			// Increase invest order such that collect payment currency gets overwritten
			// NOTE: Overwriting InvestmentPaymentCurrency works here because we manually
			// clear that state after investing with pool currency as a short cut for
			// testing purposes.
			let msg = LiquidityPoolMessage::IncreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_foreign_denominated,
			};
			assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

			// Process 100% of investment at 50% rate (1 pool currency = 2 tranche tokens)
			assert_ok!(Investments::process_invest_orders(default_investment_id()));
			assert_ok!(Investments::invest_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::one(),
					price: Ratio::checked_from_rational(1, 2).unwrap(),
				}
			));
			assert_ok!(Investments::collect_investments_for(
				RuntimeOrigin::signed(Keyring::Alice.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				InvestmentPaymentCurrency::<DevelopmentRuntime>::get(
					&investor,
					default_investment_id()
				)
				.unwrap(),
				foreign_currency
			);
			assert!(Tokens::balance(default_investment_id().into(), &investor).is_zero());
			assert_eq!(
				Tokens::balance(default_investment_id().into(), &sending_domain_locator),
				invest_amount_pool_denominated * 2
			);
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectInvest {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(foreign_currency),
							currency_payout: invest_amount_foreign_denominated,
							tranche_tokens_payout: invest_amount_pool_denominated * 2,
							remaining_invest_amount: 0,
						},
					}
					.into()
			}));

			// Should not be cleared as invest state is swapping into pool currency
			assert_eq!(
				InvestmentPaymentCurrency::<DevelopmentRuntime>::get(
					&investor,
					default_investment_id()
				)
				.unwrap(),
				foreign_currency
			);
		});
	}

	/// Invest in pool currency, then increase in allowed foreign currency, then
	/// decrease in same foreign currency multiple times.
	#[tokio::test]
	async fn invest_increase_decrease() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
			let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
			let pool_currency_decimals = currency_decimals::AUSD;
			let invest_amount_pool_denominated: u128 = 6 * dollar(18);
			create_currency_pool(pool_id, pool_currency, pool_currency_decimals.into());
			do_initial_increase_investment(
				pool_id,
				invest_amount_pool_denominated,
				investor.clone(),
				pool_currency,
				true,
			);

			// USDT investment preparations
			let invest_amount_foreign_denominated = enable_usdt_trading(
				pool_currency,
				invest_amount_pool_denominated,
				false,
				true,
				true,
				|| {
					let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
						pool_id,
						tranche_id: default_tranche_id(pool_id),
						investor: investor.clone().into(),
						currency: general_currency_index(foreign_currency),
						amount: 1,
					};
					// Should fail to increase to an invalid payment currency
					assert_noop!(
						LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, increase_msg),
						pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidPaymentCurrency
					);
				},
			);
			let increase_msg = LiquidityPoolMessage::IncreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_foreign_denominated,
			};

			// Should be able to invest since InvestmentState does not have an active swap,
			// i.e. any tradable pair is allowed to invest at this point
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				increase_msg
			));
			assert!(System::events().iter().any(|e| {
				e.event == pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignInvestmentUpdated {
					investor: investor.clone(),
					investment_id: default_investment_id(),
					state: InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
						swap: Swap {
							amount: invest_amount_pool_denominated,
							currency_in: pool_currency,
							currency_out: foreign_currency,
						},
						invest_amount: invest_amount_pool_denominated
					},
				}
					.into()
			}));

			// Should be able to to decrease in the swapping foreign currency
			enable_liquidity_pool_transferability(foreign_currency);
			let decrease_msg_pool_swap_amount = LiquidityPoolMessage::DecreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_foreign_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				decrease_msg_pool_swap_amount
			));
			// Entire swap amount into pool currency should be nullified
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing {
					invest_amount: invest_amount_pool_denominated
				}
			);
			assert!(System::events().iter().any(|e| {
				e.event ==
					pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignInvestmentUpdated
					{ 		investor: investor.clone(),
						investment_id: default_investment_id(),
						state: InvestState::InvestmentOngoing {
							invest_amount: invest_amount_pool_denominated
						},
					}
						.into()
			}));

			// Decrease partial investing amount
			enable_liquidity_pool_transferability(foreign_currency);
			let decrease_msg_partial_invest_amount = LiquidityPoolMessage::DecreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_foreign_denominated / 2,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				decrease_msg_partial_invest_amount.clone()
			));
			// Decreased amount should be taken from investing amount
			let expected_state = InvestState::ActiveSwapIntoForeignCurrencyAndInvestmentOngoing {
				swap: Swap {
					amount: invest_amount_foreign_denominated / 2,
					currency_in: foreign_currency,
					currency_out: pool_currency,
				},
				invest_amount: invest_amount_pool_denominated / 2,
			};
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				expected_state.clone()
			);
			assert!(System::events().iter().any(|e| {
				e.event ==
					pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignInvestmentUpdated
					{ 		investor: investor.clone(),
						investment_id: default_investment_id(),
						state: expected_state.clone()
					}
						.into()
			}));

			/// Consume entire investing amount by sending same message
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				decrease_msg_partial_invest_amount.clone()
			));
			let expected_state = InvestState::ActiveSwapIntoForeignCurrency {
				swap: Swap {
					amount: invest_amount_foreign_denominated,
					currency_in: foreign_currency,
					currency_out: pool_currency,
				},
			};
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				expected_state.clone()
			);
			assert!(System::events().iter().any(|e| {
				e.event ==
					pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignInvestmentUpdated
					{ 		investor: investor.clone(),
						investment_id: default_investment_id(),
						state: expected_state.clone()
					}
						.into()
			}));
		});
	}

	/// Propagate swaps only via OrderBook fulfillments.
	///
	/// Flow: Increase, fulfill, decrease, fulfill
	#[tokio::test]
	async fn invest_swaps_happy_path() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let trader: AccountId = Keyring::Alice.into();
			let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
			let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
			let pool_currency_decimals = currency_decimals::AUSD;
			let invest_amount_pool_denominated: u128 = 10 * dollar(18);
			create_currency_pool(pool_id, pool_currency, pool_currency_decimals.into());
			let invest_amount_foreign_denominated: u128 = enable_usdt_trading(
				pool_currency,
				invest_amount_pool_denominated,
				true,
				true,
				true,
				|| {},
			);
			assert_ok!(Tokens::mint_into(
				pool_currency,
				&trader,
				invest_amount_pool_denominated
			));

			// Increase such that active swap into USDT is initialized
			do_initial_increase_investment(
				pool_id,
				invest_amount_foreign_denominated,
				investor.clone(),
				foreign_currency,
				false,
			);
			let swap_order_id =
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.expect("Swap order id created during increase");
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id),
				Some(ForeignInvestmentInfo {
					owner: investor.clone(),
					id: default_investment_id(),
					last_swap_reason: Some(TokenSwapReason::Investment)
				})
			);

			// Fulfilling order should propagate it from `ActiveSwapIntoForeignCurrency` to
			// `InvestmentOngoing`.
			assert_ok!(OrderBook::fill_order_full(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id
			));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderFulfillment {
						order_id: swap_order_id,
						placing_account: investor.clone(),
						fulfilling_account: trader.clone(),
						partial_fulfillment: false,
						fulfillment_amount: invest_amount_pool_denominated,
						currency_in: pool_currency,
						currency_out: foreign_currency,
						sell_rate_limit: Ratio::one(),
					}
					.into()
			}));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing {
					invest_amount: invest_amount_pool_denominated
				}
			);
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_none()
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_none());

			// Decrease by half the investment amount
			let msg = LiquidityPoolMessage::DecreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_foreign_denominated / 2,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoForeignCurrencyAndInvestmentOngoing {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 2,
						currency_in: foreign_currency,
						currency_out: pool_currency,
					},
					invest_amount: invest_amount_pool_denominated / 2,
				}
			);
			let swap_order_id =
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.expect("Swap order id created during decrease");
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id),
				Some(ForeignInvestmentInfo {
					owner: investor.clone(),
					id: default_investment_id(),
					last_swap_reason: Some(TokenSwapReason::Investment)
				})
			);

			// Fulfill the decrease swap order
			assert_ok!(OrderBook::fill_order_full(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id
			));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderFulfillment {
						order_id: swap_order_id,
						placing_account: investor.clone(),
						fulfilling_account: trader.clone(),
						partial_fulfillment: false,
						fulfillment_amount: invest_amount_foreign_denominated / 2,
						currency_in: foreign_currency,
						currency_out: pool_currency,
						sell_rate_limit: Ratio::one(),
					}
					.into()
			}));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing {
					invest_amount: invest_amount_pool_denominated / 2
				}
			);
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_none()
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_none());
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedDecreaseInvestOrder {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(foreign_currency),
							currency_payout: invest_amount_foreign_denominated / 2,
							remaining_invest_amount: invest_amount_foreign_denominated / 2,
						},
					}
					.into()
			}));
		});
	}

	/// Verify handling concurrent swap orders works if
	/// * Invest is swapping from pool to foreign after decreasing an
	///   unprocessed investment
	/// * Redeem is swapping from pool to foreign after collecting
	#[tokio::test]
	async fn concurrent_swap_orders_same_direction() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let trader: AccountId = Keyring::Alice.into();
			let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
			let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
			let pool_currency_decimals = currency_decimals::AUSD;
			let invest_amount_pool_denominated: u128 = 10 * dollar(18);
			let swap_order_id = 1;
			create_currency_pool(pool_id, pool_currency, pool_currency_decimals.into());
			let invest_amount_foreign_denominated: u128 = enable_usdt_trading(
				pool_currency,
				invest_amount_pool_denominated,
				true,
				true,
				true,
				|| {},
			);
			// invest in pool currency to reach `InvestmentOngoing` quickly
			do_initial_increase_investment(
				pool_id,
				invest_amount_pool_denominated,
				investor.clone(),
				pool_currency,
				true,
			);
			// Manually set payment currency since we removed it in the above shortcut setup
			InvestmentPaymentCurrency::<DevelopmentRuntime>::insert(
				&investor,
				default_investment_id(),
				foreign_currency,
			);
			assert_ok!(Tokens::mint_into(
				foreign_currency,
				&trader,
				invest_amount_foreign_denominated * 2
			));

			// Decrease invest setup to have invest order swapping into foreign currency
			let msg = LiquidityPoolMessage::DecreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_foreign_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));

			// Redeem setup: Increase and process
			assert_ok!(Tokens::mint_into(
				default_investment_id().into(),
				&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
				invest_amount_pool_denominated
			));
			let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_pool_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));
			let pool_account =
				pallet_pool_system::pool_types::PoolLocator { pool_id }.into_account_truncating();
			assert_ok!(Tokens::mint_into(
				pool_currency,
				&pool_account,
				invest_amount_pool_denominated
			));
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			// Process 50% of redemption at 25% rate, i.e. 1 pool currency = 4 tranche
			// tokens
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(50),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::Investment
			);
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Charlie.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::InvestmentAndRedemption
			);
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::RedeemingAndActiveSwapIntoForeignCurrency {
					redeem_amount: invest_amount_pool_denominated / 2,
					swap: Swap {
						amount: invest_amount_foreign_denominated / 8,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			assert_eq!(
				RedemptionPayoutCurrency::<DevelopmentRuntime>::get(
					&investor,
					default_investment_id()
				)
				.unwrap(),
				foreign_currency
			);
			let swap_amount =
				invest_amount_foreign_denominated + invest_amount_foreign_denominated / 8;
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderUpdated {
						order_id: swap_order_id,
						account: investor.clone(),
						buy_amount: swap_amount,
						sell_rate_limit: Ratio::one(),
						min_fulfillment_amount: min_fulfillment_amount(foreign_currency),
					}
					.into()
			}));
			ensure_executed_collect_redeem_not_dispatched();

			// Process remaining redemption at 25% rate, i.e. 1 pool currency =
			// 4 tranche tokens
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(100),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Charlie.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 4,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			let swap_amount =
				invest_amount_foreign_denominated + invest_amount_foreign_denominated / 4;
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderUpdated {
						order_id: swap_order_id,
						account: investor.clone(),
						buy_amount: swap_amount,
						sell_rate_limit: Ratio::one(),
						min_fulfillment_amount: min_fulfillment_amount(foreign_currency),
					}
					.into()
			}));

			// Fulfilling order should kill both the invest as well as redeem state
			assert_ok!(OrderBook::fill_order_full(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id
			));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderFulfillment {
						order_id: swap_order_id,
						placing_account: investor.clone(),
						fulfilling_account: trader.clone(),
						partial_fulfillment: false,
						fulfillment_amount: invest_amount_foreign_denominated / 4 * 5,
						currency_in: foreign_currency,
						currency_out: pool_currency,
						sell_rate_limit: Ratio::one(),
					}
					.into()
			}));
			assert!(!InvestmentState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			));
			assert!(!RedemptionState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			));
			assert!(
				!RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_none());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_none()
			);
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectRedeem {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(foreign_currency),
							currency_payout: invest_amount_foreign_denominated / 4,
							tranche_tokens_payout: invest_amount_pool_denominated,
							remaining_redeem_amount: 0,
						},
					}
					.into()
			}));
		});
	}

	/// Verify handling concurrent swap orders works if
	/// * Invest is swapping from foreign to pool after increasing
	/// * Redeem is swapping from pool to foreign after collecting
	#[tokio::test]
	async fn concurrent_swap_orders_opposite_direction() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let trader: AccountId = Keyring::Alice.into();
			let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
			let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
			let pool_currency_decimals = currency_decimals::AUSD;
			let invest_amount_pool_denominated: u128 = 10 * dollar(18);
			let swap_order_id = 1;
			create_currency_pool(pool_id, pool_currency, pool_currency_decimals.into());
			let invest_amount_foreign_denominated: u128 = enable_usdt_trading(
				pool_currency,
				invest_amount_pool_denominated,
				true,
				true,
				true,
				|| {},
			);
			assert_ok!(Tokens::mint_into(
				foreign_currency,
				&trader,
				invest_amount_foreign_denominated * 2
			));

			// Increase invest setup to have invest order swapping into pool currency
			do_initial_increase_investment(
				pool_id,
				invest_amount_foreign_denominated,
				investor.clone(),
				foreign_currency,
				false,
			);
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoPoolCurrency {
					swap: Swap {
						amount: invest_amount_pool_denominated,
						currency_in: pool_currency,
						currency_out: foreign_currency
					}
				},
			);
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::Investment
			);
			assert_eq!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id()),
				Some(swap_order_id)
			);

			// Redeem setup: Increase and process
			assert_ok!(Tokens::mint_into(
				default_investment_id().into(),
				&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
				3 * invest_amount_pool_denominated
			));
			let pool_account =
				pallet_pool_system::pool_types::PoolLocator { pool_id }.into_account_truncating();
			assert_ok!(Tokens::mint_into(
				pool_currency,
				&pool_account,
				3 * invest_amount_pool_denominated
			));
			let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_pool_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::Investment
			);
			assert_eq!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id()),
				Some(swap_order_id)
			);

			// Process 50% of redemption at 25% rate, i.e. 1 pool currency = 4 tranche
			// tokens
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(50),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Charlie.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::Investment
			);
			assert_eq!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id()),
				Some(swap_order_id)
			);
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
					invest_amount: invest_amount_pool_denominated / 8,
					swap: Swap {
						amount: invest_amount_pool_denominated / 8 * 7,
						currency_in: pool_currency,
						currency_out: foreign_currency
					}
				},
			);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::Redeeming {
					redeem_amount: invest_amount_pool_denominated / 2,
				}
			);

			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderUpdated {
						order_id: swap_order_id,
						account: investor.clone(),
						buy_amount: invest_amount_pool_denominated / 8 * 7,
						sell_rate_limit: Ratio::one(),
						min_fulfillment_amount: min_fulfillment_amount(pool_currency),
					}
					.into()
			}));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectRedeem {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(foreign_currency),
							currency_payout: invest_amount_foreign_denominated / 8,
							tranche_tokens_payout: invest_amount_pool_denominated / 2,
							remaining_redeem_amount: invest_amount_pool_denominated / 2,
						},
					}
					.into()
			}));

			// Process remaining redemption at 25% rate, i.e. 1 pool currency =
			// 4 tranche tokens
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(100),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Charlie.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
					invest_amount: invest_amount_pool_denominated / 4,
					swap: Swap {
						amount: invest_amount_pool_denominated / 4 * 3,
						currency_in: pool_currency,
						currency_out: foreign_currency
					}
				}
			);
			assert!(!RedemptionState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderUpdated {
						order_id: swap_order_id,
						account: investor.clone(),
						buy_amount: invest_amount_pool_denominated / 4 * 3,
						sell_rate_limit: Ratio::one(),
						min_fulfillment_amount: min_fulfillment_amount(pool_currency),
					}
					.into()
			}));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectRedeem {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(foreign_currency),
							currency_payout: invest_amount_foreign_denominated / 8,
							tranche_tokens_payout: invest_amount_pool_denominated / 2,
							remaining_redeem_amount: 0,
						},
					}
					.into()
			}));

			// Redeem again with goal of redemption swap to foreign consuming investment
			// swap to pool
			let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_pool_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));
			// Process remaining redemption at 200% rate, i.e. 1 tranche token = 2 pool
			// currency
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(100),
					price: Ratio::checked_from_rational(2, 1).unwrap(),
				}
			));
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Charlie.into()),
				investor.clone(),
				default_investment_id()
			));
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_none());
			// Swap order id should be bumped since swap order update occurred for opposite
			// direction (from foreign->pool to foreign->pool)
			let swap_order_id = 2;
			assert_eq!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id()),
				Some(swap_order_id)
			);
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::Redemption
			);
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing {
					invest_amount: invest_amount_pool_denominated
				}
			);
			let remaining_foreign_swap_amount =
				2 * invest_amount_foreign_denominated - invest_amount_foreign_denominated / 4 * 3;
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					done_amount: invest_amount_foreign_denominated / 4 * 3,
					swap: Swap {
						amount: remaining_foreign_swap_amount,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			ensure_executed_collect_redeem_not_dispatched();

			// Fulfilling order should the invest
			assert_ok!(OrderBook::fill_order_full(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id
			));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderFulfillment {
						order_id: swap_order_id,
						placing_account: investor.clone(),
						fulfilling_account: trader.clone(),
						partial_fulfillment: false,
						fulfillment_amount: remaining_foreign_swap_amount,
						currency_in: foreign_currency,
						currency_out: pool_currency,
						sell_rate_limit: Ratio::one(),
					}
					.into()
			}));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing {
					invest_amount: invest_amount_pool_denominated
				}
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_none());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_none()
			);
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectRedeem {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(foreign_currency),
							currency_payout: invest_amount_foreign_denominated * 2,
							tranche_tokens_payout: invest_amount_pool_denominated,
							remaining_redeem_amount: 0,
						},
					}
					.into()
			}));
		});
	}

	/// 1. increase initial invest in pool currency
	/// 2. increase invest in foreign
	/// 3. process invest
	/// 4. fulfill swap order
	#[tokio::test]
	async fn fulfill_invest_swap_order_requires_collect() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let trader: AccountId = Keyring::Alice.into();
			let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
			let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
			let pool_currency_decimals = currency_decimals::AUSD;
			let invest_amount_pool_denominated: u128 = 10 * dollar(18);
			let swap_order_id = 1;
			create_currency_pool(pool_id, pool_currency, pool_currency_decimals.into());
			let invest_amount_foreign_denominated: u128 = enable_usdt_trading(
				pool_currency,
				invest_amount_pool_denominated,
				true,
				true,
				true,
				|| {},
			);
			// invest in pool currency to reach `InvestmentOngoing` quickly
			do_initial_increase_investment(
				pool_id,
				invest_amount_pool_denominated,
				investor.clone(),
				pool_currency,
				true,
			);
			assert_ok!(Tokens::mint_into(
				pool_currency,
				&trader,
				invest_amount_pool_denominated
			));

			// Increase invest have
			// InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing
			let msg = LiquidityPoolMessage::IncreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_foreign_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
					swap: Swap {
						amount: invest_amount_pool_denominated,
						currency_in: pool_currency,
						currency_out: foreign_currency,
					},
					invest_amount: invest_amount_pool_denominated
				}
			);
			// Process 50% of investment at 25% rate, i.e. 1 pool currency = 4 tranche
			// tokens
			assert_ok!(Investments::process_invest_orders(default_investment_id()));
			assert_ok!(Investments::invest_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(50),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));
			assert!(Investments::investment_requires_collect(
				&investor,
				default_investment_id()
			));

			// Fulfill swap order should implicitly collect, otherwise the unprocessed
			// investment amount is unknown
			assert_ok!(OrderBook::fill_order_full(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id
			));
			assert!(!Investments::investment_requires_collect(
				&investor,
				default_investment_id()
			));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing {
					invest_amount: invest_amount_pool_denominated / 2 * 3
				}
			);
		});
	}

	/// 1. increase initial redeem
	/// 2. process partial redemption
	/// 3. collect
	/// 4. process redemption
	/// 5. fulfill swap order should implicitly collect
	#[tokio::test]
	async fn fulfill_redeem_swap_order_requires_collect() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			let pool_id = DEFAULT_POOL_ID;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let trader: AccountId = Keyring::Alice.into();
			let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
			let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
			let pool_currency_decimals = currency_decimals::AUSD;
			let invest_amount_pool_denominated: u128 = 10 * dollar(18);
			let swap_order_id = 1;
			create_currency_pool(pool_id, pool_currency, pool_currency_decimals.into());
			let invest_amount_foreign_denominated: u128 = enable_usdt_trading(
				pool_currency,
				invest_amount_pool_denominated,
				true,
				true,
				true,
				|| {},
			);
			// invest in pool currency to reach `InvestmentOngoing` quickly
			do_initial_increase_investment(
				pool_id,
				invest_amount_pool_denominated,
				investor.clone(),
				pool_currency,
				true,
			);
			// Manually set payment currency since we removed it in the above shortcut setup
			InvestmentPaymentCurrency::<DevelopmentRuntime>::insert(
				&investor,
				default_investment_id(),
				foreign_currency,
			);
			assert_ok!(Tokens::mint_into(
				foreign_currency,
				&trader,
				invest_amount_foreign_denominated * 2
			));

			// Decrease invest setup to have invest order swapping into foreign currency
			let msg = LiquidityPoolMessage::DecreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_foreign_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));

			// Redeem setup: Increase and process
			assert_ok!(Tokens::mint_into(
				default_investment_id().into(),
				&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
				invest_amount_pool_denominated
			));
			let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_pool_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));
			let pool_account =
				pallet_pool_system::pool_types::PoolLocator { pool_id }.into_account_truncating();
			assert_ok!(Tokens::mint_into(
				pool_currency,
				&pool_account,
				invest_amount_pool_denominated
			));
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			// Process 50% of redemption at 25% rate, i.e. 1 pool currency = 4 tranche
			// tokens
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(50),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::Investment
			);
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Charlie.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::InvestmentAndRedemption
			);
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::RedeemingAndActiveSwapIntoForeignCurrency {
					redeem_amount: invest_amount_pool_denominated / 2,
					swap: Swap {
						amount: invest_amount_foreign_denominated / 8,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			assert_eq!(
				RedemptionPayoutCurrency::<DevelopmentRuntime>::get(
					&investor,
					default_investment_id()
				)
				.unwrap(),
				foreign_currency
			);
			let swap_amount =
				invest_amount_foreign_denominated + invest_amount_foreign_denominated / 8;
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderUpdated {
						order_id: swap_order_id,
						account: investor.clone(),
						buy_amount: swap_amount,
						sell_rate_limit: Ratio::one(),
						min_fulfillment_amount: min_fulfillment_amount(foreign_currency),
					}
					.into()
			}));
			ensure_executed_collect_redeem_not_dispatched();

			// Process remaining redemption at 25% rate, i.e. 1 pool currency =
			// 4 tranche tokens
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(100),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Charlie.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 4,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			let swap_amount =
				invest_amount_foreign_denominated + invest_amount_foreign_denominated / 4;
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderUpdated {
						order_id: swap_order_id,
						account: investor.clone(),
						buy_amount: swap_amount,
						sell_rate_limit: Ratio::one(),
						min_fulfillment_amount: min_fulfillment_amount(foreign_currency),
					}
					.into()
			}));

			// Partially fulfilling the swap order below the invest swapping amount should
			// still have both states swapping into foreign
			assert_ok!(OrderBook::fill_order_partial(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id,
				invest_amount_foreign_denominated / 2
			));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderFulfillment {
						order_id: swap_order_id,
						placing_account: investor.clone(),
						fulfilling_account: trader.clone(),
						partial_fulfillment: true,
						fulfillment_amount: invest_amount_foreign_denominated / 2,
						currency_in: foreign_currency,
						currency_out: pool_currency,
						sell_rate_limit: Ratio::one(),
					}
					.into()
			}));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 2,
						currency_in: foreign_currency,
						currency_out: pool_currency
					},
					done_amount: invest_amount_foreign_denominated / 2
				}
			);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 4,
						currency_in: foreign_currency,
						currency_out: pool_currency
					},
				}
			);
			assert!(
				RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_some());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_some()
			);
			ensure_executed_collect_redeem_not_dispatched();

			// Partially fulfilling the swap order for the remaining invest swap amount
			// should still clear the investment state
			assert_ok!(OrderBook::fill_order_partial(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id,
				invest_amount_foreign_denominated / 2
			));
			assert!(!InvestmentState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 4,
						currency_in: foreign_currency,
						currency_out: pool_currency
					},
				}
			);
			assert!(
				RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_some());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_some()
			);
			ensure_executed_collect_redeem_not_dispatched();

			// Partially fulfilling the swap order below the redeem swap amount should still
			// clear the investment state
			assert_ok!(OrderBook::fill_order_partial(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id,
				invest_amount_foreign_denominated / 8
			));
			assert!(!InvestmentState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 8,
						currency_in: foreign_currency,
						currency_out: pool_currency
					},
					done_amount: invest_amount_foreign_denominated / 8
				}
			);
			assert!(
				RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_some());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_some()
			);
			ensure_executed_collect_redeem_not_dispatched();

			// Partially fulfilling the swap order below the redeem swap amount should still
			// clear the investment state
			assert_ok!(OrderBook::fill_order_partial(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id,
				invest_amount_foreign_denominated / 8
			));
			assert!(!InvestmentState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert!(!RedemptionState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert!(
				!RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_none());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_none()
			);
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectRedeem {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(foreign_currency),
							currency_payout: invest_amount_foreign_denominated / 4,
							tranche_tokens_payout: invest_amount_pool_denominated,
							remaining_redeem_amount: 0,
						},
					}
					.into()
			}));
		});
	}

	/// Similar to [concurrent_swap_orders_same_direction] but with partial
	/// fulfillment
	#[tokio::test]
	async fn partial_fulfillment_concurrent_swap_orders_same_direction() {
		let mut env = {
			let mut genesis = Storage::default();
			genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
			env::test_env_with_centrifuge_storage(Handle::current(), genesis)
		};

		setup_test_env(&mut env);

		env.with_mut_state(Chain::Para(PARA_ID), || {
			// Increase invest setup
			let pool_id = DEFAULT_POOL_ID;
			let investor: AccountId =
				AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert((
					DOMAIN_MOONBEAM,
					Keyring::Bob.into(),
				));
			let trader: AccountId = Keyring::Alice.into();
			let pool_currency: CurrencyId = AUSD_CURRENCY_ID;
			let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
			let pool_currency_decimals = currency_decimals::AUSD;
			let invest_amount_pool_denominated: u128 = 10 * dollar(18);
			let swap_order_id = 1;
			create_currency_pool(pool_id, pool_currency, pool_currency_decimals.into());
			let invest_amount_foreign_denominated: u128 = enable_usdt_trading(
				pool_currency,
				invest_amount_pool_denominated,
				true,
				true,
				true,
				|| {},
			);
			// invest in pool currency to reach `InvestmentOngoing` quickly
			do_initial_increase_investment(
				pool_id,
				invest_amount_pool_denominated,
				investor.clone(),
				pool_currency,
				true,
			);
			// Manually set payment currency since we removed it in the above shortcut setup
			InvestmentPaymentCurrency::<DevelopmentRuntime>::insert(
				&investor,
				default_investment_id(),
				foreign_currency,
			);
			assert_ok!(Tokens::mint_into(
				foreign_currency,
				&trader,
				invest_amount_foreign_denominated * 2
			));

			// Decrease invest setup to have invest order swapping into foreign currency
			let msg = LiquidityPoolMessage::DecreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_foreign_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));

			// Redeem setup: Increase and process
			assert_ok!(Tokens::mint_into(
				default_investment_id().into(),
				&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
				invest_amount_pool_denominated
			));
			let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(foreign_currency),
				amount: invest_amount_pool_denominated,
			};
			assert_ok!(LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg.clone()
			));
			let pool_account =
				pallet_pool_system::pool_types::PoolLocator { pool_id }.into_account_truncating();
			assert_ok!(Tokens::mint_into(
				pool_currency,
				&pool_account,
				invest_amount_pool_denominated
			));
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			// Process 50% of redemption at 25% rate, i.e. 1 pool currency = 4 tranche
			// tokens
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(50),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::Investment
			);
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Charlie.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				ForeignInvestments::foreign_investment_info(swap_order_id)
					.unwrap()
					.last_swap_reason
					.unwrap(),
				TokenSwapReason::InvestmentAndRedemption
			);
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::RedeemingAndActiveSwapIntoForeignCurrency {
					redeem_amount: invest_amount_pool_denominated / 2,
					swap: Swap {
						amount: invest_amount_foreign_denominated / 8,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			assert_eq!(
				RedemptionPayoutCurrency::<DevelopmentRuntime>::get(
					&investor,
					default_investment_id()
				)
				.unwrap(),
				foreign_currency
			);
			let swap_amount =
				invest_amount_foreign_denominated + invest_amount_foreign_denominated / 8;
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderUpdated {
						order_id: swap_order_id,
						account: investor.clone(),
						buy_amount: swap_amount,
						sell_rate_limit: Ratio::one(),
						min_fulfillment_amount: min_fulfillment_amount(foreign_currency),
					}
					.into()
			}));
			ensure_executed_collect_redeem_not_dispatched();

			// Process remaining redemption at 25% rate, i.e. 1 pool currency =
			// 4 tranche tokens
			assert_ok!(Investments::process_redeem_orders(default_investment_id()));
			assert_ok!(Investments::redeem_fulfillment(
				default_investment_id(),
				FulfillmentWithPrice {
					of_amount: Perquintill::from_percent(100),
					price: Ratio::checked_from_rational(1, 4).unwrap(),
				}
			));
			assert_ok!(Investments::collect_redemptions_for(
				RuntimeOrigin::signed(Keyring::Charlie.into()),
				investor.clone(),
				default_investment_id()
			));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 4,
						currency_in: foreign_currency,
						currency_out: pool_currency
					}
				}
			);
			let swap_amount =
				invest_amount_foreign_denominated + invest_amount_foreign_denominated / 4;
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderUpdated {
						order_id: swap_order_id,
						account: investor.clone(),
						buy_amount: swap_amount,
						sell_rate_limit: Ratio::one(),
						min_fulfillment_amount: min_fulfillment_amount(foreign_currency),
					}
					.into()
			}));

			// Partially fulfilling the swap order below the invest swapping amount should
			// still have both states swapping into foreign
			assert_ok!(OrderBook::fill_order_partial(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id,
				invest_amount_foreign_denominated / 2
			));
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_order_book::Event::<DevelopmentRuntime>::OrderFulfillment {
						order_id: swap_order_id,
						placing_account: investor.clone(),
						fulfilling_account: trader.clone(),
						partial_fulfillment: true,
						fulfillment_amount: invest_amount_foreign_denominated / 2,
						currency_in: foreign_currency,
						currency_out: pool_currency,
						sell_rate_limit: Ratio::one(),
					}
					.into()
			}));
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 2,
						currency_in: foreign_currency,
						currency_out: pool_currency
					},
					done_amount: invest_amount_foreign_denominated / 2
				}
			);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 4,
						currency_in: foreign_currency,
						currency_out: pool_currency
					},
				}
			);
			assert!(
				RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_some());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_some()
			);
			ensure_executed_collect_redeem_not_dispatched();

			// Partially fulfilling the swap order for the remaining invest swap amount
			// should still clear the investment state
			assert_ok!(OrderBook::fill_order_partial(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id,
				invest_amount_foreign_denominated / 2
			));
			assert!(!InvestmentState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrency {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 4,
						currency_in: foreign_currency,
						currency_out: pool_currency
					},
				}
			);
			assert!(
				RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_some());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_some()
			);
			ensure_executed_collect_redeem_not_dispatched();

			// Partially fulfilling the swap order below the redeem swap amount should still
			// clear the investment state
			assert_ok!(OrderBook::fill_order_partial(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id,
				invest_amount_foreign_denominated / 8
			));
			assert!(!InvestmentState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert_eq!(
				RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				RedeemState::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					swap: Swap {
						amount: invest_amount_foreign_denominated / 8,
						currency_in: foreign_currency,
						currency_out: pool_currency
					},
					done_amount: invest_amount_foreign_denominated / 8
				}
			);
			assert!(
				RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_some());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_some()
			);
			ensure_executed_collect_redeem_not_dispatched();

			// Partially fulfilling the swap order below the redeem swap amount should still
			// clear the investment state
			assert_ok!(OrderBook::fill_order_partial(
				RuntimeOrigin::signed(trader.clone()),
				swap_order_id,
				invest_amount_foreign_denominated / 8
			));
			assert!(!InvestmentState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert!(!RedemptionState::<DevelopmentRuntime>::contains_key(
				&investor,
				default_investment_id()
			),);
			assert!(
				!RedemptionPayoutCurrency::<DevelopmentRuntime>::contains_key(
					&investor,
					default_investment_id()
				)
			);
			assert!(ForeignInvestments::foreign_investment_info(swap_order_id).is_none());
			assert!(
				ForeignInvestments::token_swap_order_ids(&investor, default_investment_id())
					.is_none()
			);
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						sender: TreasuryAccount::get(),
						domain: DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain(),
						message: pallet_liquidity_pools::Message::ExecutedCollectRedeem {
							pool_id,
							tranche_id: default_tranche_id(pool_id),
							investor: investor.clone().into(),
							currency: general_currency_index(foreign_currency),
							currency_payout: invest_amount_foreign_denominated / 4,
							tranche_tokens_payout: invest_amount_pool_denominated,
							remaining_redeem_amount: 0,
						},
					}
					.into()
			}));
		});
	}
}

mod setup {
	use cfg_traits::{investments::ForeignInvestment, ConversionToAssetBalance};
	use development_runtime::OrderBook;

	use super::*;
	use crate::{
		liquidity_pools::pallet::development::tests::{
			liquidity_pools::setup::DEFAULT_OTHER_DOMAIN_ADDRESS, register_usdt,
		},
		utils::USDT_CURRENCY_ID,
	};

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
		clear_investment_payment_currency: bool,
	) {
		let valid_until = DEFAULT_VALIDITY;
		let pool_currency: CurrencyId =
			PoolSystem::currency_for(pool_id).expect("Pool existence checked already");

		// Mock incoming increase invest message
		let msg = LiquidityPoolMessage::IncreaseInvestOrder {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
			amount,
		};

		// Should fail if investor does not have investor role yet
		// However, failure is async for foreign currencies as part of updating the
		// investment after the swap was fulfilled
		if currency_id == pool_currency {
			assert_noop!(
				LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
				DispatchError::Other("Account does not have the TrancheInvestor permission.")
			);
		}

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

		let amount_before = Tokens::balance(currency_id, &default_investment_account());
		let final_amount = amount_before
			.ensure_add(amount)
			.expect("Should not overflow when incrementing amount");

		// Execute byte message
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));
		assert_eq!(
			InvestmentPaymentCurrency::<DevelopmentRuntime>::get(
				&investor,
				default_investment_id()
			)
			.unwrap(),
			currency_id,
		);

		if currency_id == pool_currency {
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::InvestmentOngoing {
					invest_amount: amount
				}
			);
			// Verify investment was transferred into investment account
			assert_eq!(
				Tokens::balance(currency_id, &default_investment_account()),
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
			assert!(System::events().iter().any(|e| {
				e.event
					== pallet_investments::Event::<DevelopmentRuntime>::InvestOrderUpdated {
						investment_id: default_investment_id(),
						submitted_at: 0,
						who: investor.clone(),
						amount: final_amount,
					}
					.into()
			}));
		} else {
			let amount_pool_denominated: u128 =
				IdentityPoolCurrencyConverter::<OrmlAssetRegistry>::stable_to_stable(
					pool_currency,
					currency_id,
					amount,
				)
				.unwrap();
			assert_eq!(
				InvestmentState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
				InvestState::ActiveSwapIntoPoolCurrency {
					swap: Swap {
						currency_in: pool_currency,
						currency_out: currency_id,
						amount: amount_pool_denominated
					}
				}
			);
		}

		// NOTE: In some tests, we run this setup with a pool currency to immediately
		// set the investment state to `InvestmentOngoing`. However, afterwards we want
		// to invest with another currency and treat that investment as the initial one.
		// In order to do that, we need to clear the payment currency.
		if clear_investment_payment_currency {
			InvestmentPaymentCurrency::<DevelopmentRuntime>::remove(
				&investor,
				default_investment_id(),
			);
		}
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
		assert_ok!(Tokens::mint_into(
			default_investment_id().into(),
			&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
			amount
		));

		// Verify redemption has not been made yet
		assert_eq!(
			Tokens::balance(
				default_investment_id().into(),
				&default_investment_account(),
			),
			0
		);
		assert_eq!(
			Tokens::balance(default_investment_id().into(), &investor),
			0
		);

		// Mock incoming increase invest message
		let msg = LiquidityPoolMessage::IncreaseRedeemOrder {
			pool_id: pool_id,
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

		assert_eq!(
			RedemptionState::<DevelopmentRuntime>::get(&investor, default_investment_id()),
			RedeemState::Redeeming {
				redeem_amount: amount
			}
		);
		assert_eq!(
			RedemptionPayoutCurrency::<DevelopmentRuntime>::get(&investor, default_investment_id())
				.unwrap(),
			currency_id
		);
		// Verify redemption was transferred into investment account
		assert_eq!(
			Tokens::balance(
				default_investment_id().into(),
				&default_investment_account(),
			),
			amount
		);
		assert_eq!(
			Tokens::balance(default_investment_id().into(), &investor),
			0
		);
		assert_eq!(
			Tokens::balance(
				default_investment_id().into(),
				&AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert(
					DEFAULT_OTHER_DOMAIN_ADDRESS
				)
			),
			0
		);
		assert_eq!(
			System::events().iter().nth_back(4).unwrap().event,
			pallet_foreign_investments::Event::<DevelopmentRuntime>::ForeignRedemptionUpdated {
				investor: investor.clone(),
				investment_id: default_investment_id(),
				state: RedeemState::Redeeming {
					redeem_amount: amount
				}
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

	/// Registers USDT currency, adds bidirectional trading pairs and returns
	/// the amount in foreign denomination
	pub(crate) fn enable_usdt_trading(
		pool_currency: CurrencyId,
		amount_pool_denominated: Balance,
		enable_lp_transferability: bool,
		enable_foreign_to_pool_pair: bool,
		enable_pool_to_foreign_pair: bool,
		pre_add_trading_pair_check: impl FnOnce() -> (),
	) -> Balance {
		register_usdt();
		let foreign_currency: CurrencyId = USDT_CURRENCY_ID;
		let amount_foreign_denominated: u128 =
			IdentityPoolCurrencyConverter::<OrmlAssetRegistry>::stable_to_stable(
				foreign_currency,
				pool_currency,
				amount_pool_denominated,
			)
			.unwrap();

		if enable_lp_transferability {
			enable_liquidity_pool_transferability(foreign_currency);
		}

		pre_add_trading_pair_check();

		if enable_foreign_to_pool_pair {
			assert!(!ForeignInvestments::accepted_payment_currency(
				default_investment_id(),
				foreign_currency
			));
			assert_ok!(OrderBook::add_trading_pair(
				RuntimeOrigin::root(),
				pool_currency,
				foreign_currency,
				1
			));
			assert!(ForeignInvestments::accepted_payment_currency(
				default_investment_id(),
				foreign_currency
			));
		}
		if enable_pool_to_foreign_pair {
			assert!(!ForeignInvestments::accepted_payout_currency(
				default_investment_id(),
				foreign_currency
			));

			assert_ok!(OrderBook::add_trading_pair(
				RuntimeOrigin::root(),
				foreign_currency,
				pool_currency,
				1
			));
			assert!(ForeignInvestments::accepted_payout_currency(
				default_investment_id(),
				foreign_currency
			));
		}

		amount_foreign_denominated
	}

	pub(crate) fn ensure_executed_collect_redeem_not_dispatched() {
		assert!(System::events().iter().any(|e| {
			match &e.event {
				development_runtime::RuntimeEvent::LiquidityPoolsGateway(
					pallet_liquidity_pools_gateway::Event::OutboundMessageSubmitted {
						message, ..
					},
				) => match message {
					pallet_liquidity_pools::Message::ExecutedCollectRedeem { .. } => false,
					_ => true,
				},
				_ => true,
			}
		}));
	}

	pub(crate) fn min_fulfillment_amount(currency_id: CurrencyId) -> Balance {
		runtime_common::foreign_investments::NativeBalanceDecimalConverter::<OrmlAssetRegistry>::to_asset_balance(
			MinFulfillmentAmountNative::get(),
			currency_id,
		)
		.expect("CurrencyId should be registered in AssetRegistry")
	}
}
