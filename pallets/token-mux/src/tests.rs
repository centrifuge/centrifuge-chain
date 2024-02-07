// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

use cfg_types::tokens::LocalAssetId;
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;

use crate::{
	mock::{
		new_test_ext, AccountId, Balance, CurrencyId, MockTokenSwaps, OrmlTokens, Runtime,
		RuntimeEvent, RuntimeOrigin, SwapId, System, TokenMux, USDC_DECIMALS,
	},
	Error,
	Event::{Burned, Deposited},
};

const ORDER_ID: SwapId = 1;
pub const USDC_1: CurrencyId = CurrencyId::ForeignAsset(1);
pub const USDC_2: CurrencyId = CurrencyId::ForeignAsset(2);
pub const NON_USDC: CurrencyId = CurrencyId::ForeignAsset(4);
pub const UNREGISTERED_ASSET: CurrencyId = CurrencyId::ForeignAsset(5);

pub const USDC_LOCAL_ASSET_ID: LocalAssetId = LocalAssetId(1u32);
pub const USDC_LOCAL: CurrencyId = CurrencyId::LocalAsset(USDC_LOCAL_ASSET_ID);

pub const USER_1: AccountId = 1;
pub const USER_2: AccountId = 2;
pub const USER_NON: AccountId = 4;
pub const USER_UNREGISTERED: AccountId = 5;
pub const USER_LOCAL: AccountId = 6;

pub const INITIAL_AMOUNT: Balance = token(1000);
pub const fn token(amount: Balance) -> Balance {
	amount * (10 as Balance).pow(USDC_DECIMALS)
}
const AMOUNT: Balance = token(10);

mod deposit {
	use super::*;

	#[test]
	fn deposit_usdc_variant_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(OrmlTokens::free_balance(USDC_1, &USER_1), INITIAL_AMOUNT);
			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_1), 0);
			assert_eq!(
				OrmlTokens::free_balance(USDC_LOCAL, &TokenMux::account()),
				0
			);

			assert_ok!(TokenMux::deposit(
				RuntimeOrigin::signed(USER_1),
				USDC_1,
				AMOUNT
			));

			assert_eq!(
				OrmlTokens::free_balance(USDC_1, &USER_1),
				INITIAL_AMOUNT - AMOUNT
			);
			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_1), AMOUNT);
			assert_eq!(
				OrmlTokens::free_balance(USDC_1, &TokenMux::account()),
				AMOUNT
			);

			System::assert_last_event(RuntimeEvent::TokenMux(Deposited {
				who: USER_1,
				currency_out: USDC_1,
				currency_in: USDC_LOCAL,
				amount: AMOUNT,
			}));
		})
	}

	#[test]
	/// 1. USER_1: Deposit USDC_1 for LOCAL
	/// 2. USER_2: Deposit USDC_2 for LOCAL
	/// 3. USER_1: Burn LOCAL for USDC_2
	/// 4. USER_2: Burn LOCAL for USDC_1
	fn deposit_burn_ring() {
		new_test_ext().execute_with(|| {
			assert_ok!(TokenMux::deposit(
				RuntimeOrigin::signed(USER_1),
				USDC_1,
				AMOUNT
			));
			assert_ok!(TokenMux::deposit(
				RuntimeOrigin::signed(USER_2),
				USDC_2,
				AMOUNT
			));
			assert_ok!(TokenMux::burn(
				RuntimeOrigin::signed(USER_1),
				USDC_2,
				AMOUNT
			));
			assert_ok!(TokenMux::burn(
				RuntimeOrigin::signed(USER_2),
				USDC_1,
				AMOUNT
			));

			assert_eq!(OrmlTokens::free_balance(USDC_2, &USER_1), AMOUNT);
			assert_eq!(OrmlTokens::free_balance(USDC_1, &USER_2), AMOUNT);
			assert_eq!(
				OrmlTokens::free_balance(USDC_LOCAL, &TokenMux::account()),
				0
			);
		})
	}

	#[test]
	fn deposit_local_usdc_throws() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::deposit(RuntimeOrigin::signed(USER_LOCAL), USDC_LOCAL, 1),
				Error::<Runtime>::NoLocalRepresentation
			);
		})
	}

	#[test]
	fn deposit_no_local_representation() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::deposit(RuntimeOrigin::signed(USER_NON), NON_USDC, 1),
				Error::<Runtime>::NoLocalRepresentation
			);
		})
	}

	#[test]
	fn deposit_missing_metadata() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::deposit(
					RuntimeOrigin::signed(USER_UNREGISTERED),
					UNREGISTERED_ASSET,
					1
				),
				Error::<Runtime>::MetadataNotFound
			);
		})
	}

	#[test]
	fn deposit_insufficient_balance() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::deposit(RuntimeOrigin::signed(USER_1), USDC_2, 1),
				orml_tokens::Error::<Runtime>::BalanceTooLow
			);
		})
	}
}

mod burn {
	use super::*;

	#[test]
	fn burn_usdc_variant_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(TokenMux::deposit(
				RuntimeOrigin::signed(USER_1),
				USDC_1,
				AMOUNT
			));
			assert_ok!(TokenMux::burn(
				RuntimeOrigin::signed(USER_1),
				USDC_1,
				AMOUNT / 2
			));

			assert_eq!(
				OrmlTokens::free_balance(USDC_1, &USER_1),
				INITIAL_AMOUNT - AMOUNT / 2
			);
			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_1), AMOUNT / 2);
			assert_eq!(
				OrmlTokens::free_balance(USDC_1, &TokenMux::account()),
				AMOUNT / 2
			);

			System::assert_last_event(RuntimeEvent::TokenMux(Burned {
				who: USER_1,
				currency_out: USDC_LOCAL,
				currency_in: USDC_1,
				amount: AMOUNT / 2,
			}));
		})
	}

	#[test]
	fn burn_local_usdc_throws() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::burn(RuntimeOrigin::signed(USER_LOCAL), USDC_LOCAL, 1),
				Error::<Runtime>::NoLocalRepresentation
			);
		})
	}

	#[test]
	fn burn_no_local_representation() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::burn(RuntimeOrigin::signed(USER_NON), NON_USDC, 1),
				Error::<Runtime>::NoLocalRepresentation
			);
		})
	}

	#[test]
	fn burn_missing_metadata() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::burn(
					RuntimeOrigin::signed(USER_UNREGISTERED),
					UNREGISTERED_ASSET,
					1
				),
				Error::<Runtime>::MetadataNotFound
			);
		})
	}

	#[test]
	fn burn_funds_unavailable() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::burn(RuntimeOrigin::signed(USER_1), USDC_1, 1),
				sp_runtime::TokenError::FundsUnavailable
			);
		})
	}
}

pub(crate) mod try_local {
	use cfg_types::tokens::LocalAssetId;

	use super::*;
	use crate::mock::new_test_ext_invalid_assets;

	pub const HAS_UNREGISTERED_LOCAL_ASSET: CurrencyId = CurrencyId::ForeignAsset(6);
	pub const USDC_WRONG_DECIMALS: CurrencyId = CurrencyId::ForeignAsset(7);
	pub const UNREGISTERED_LOCAL_ASSET_ID: LocalAssetId = LocalAssetId(2u32);
	#[test]
	fn try_local_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(TokenMux::try_local(&USDC_1), Ok(USDC_LOCAL));
		})
	}
	#[test]
	fn try_local_metadata_not_found() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::try_local(&UNREGISTERED_ASSET),
				Error::<Runtime>::MetadataNotFound
			);
		})
	}

	#[test]
	fn try_local_no_local_representation() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				TokenMux::try_local(&NON_USDC),
				Error::<Runtime>::NoLocalRepresentation
			);
		})
	}

	#[test]
	fn try_local_local_metadata_not_found() {
		new_test_ext_invalid_assets().execute_with(|| {
			assert_noop!(
				TokenMux::try_local(&HAS_UNREGISTERED_LOCAL_ASSET),
				Error::<Runtime>::MetadataNotFound
			);
		})
	}
	#[test]
	fn try_local_local_decimal_mismatch() {
		new_test_ext_invalid_assets().execute_with(|| {
			assert_noop!(
				TokenMux::try_local(&USDC_WRONG_DECIMALS),
				Error::<Runtime>::MetadataNotFound
			);
		})
	}
}

mod swaps {
	use cfg_types::orders::OrderInfo;

	use super::*;
	use crate::{
		tests::swaps::utils::{
			mock_order_details, mock_pending_swap_from_local, mock_pending_swap_to_local,
		},
		Event::SwapMatched,
	};

	mod utils {
		use cfg_traits::OrderRatio;
		use cfg_types::investments::Swap;
		use frame_support::traits::tokens::{fungibles::Mutate, Preservation};
		use sp_arithmetic::traits::One;

		use super::*;
		use crate::mock::{AccountId, Ratio};

		pub fn mock_order_details(currency_in: CurrencyId, currency_out: CurrencyId) {
			MockTokenSwaps::mock_get_order_details(move |order_id| {
				assert_eq!(order_id, ORDER_ID);
				Some(OrderInfo {
					swap: Swap {
						currency_in,
						currency_out,
						amount_out: AMOUNT,
					},
					ratio: OrderRatio::Custom(Ratio::one()),
				})
			});
		}

		pub fn mock_fill_order(
			currency_1: CurrencyId,
			account_1: AccountId,
			currency_2: CurrencyId,
			account_2: AccountId,
		) {
			MockTokenSwaps::mock_fill_order(move |_who, order_id, amount_out| {
				assert_eq!(order_id, ORDER_ID);
				assert_eq!(amount_out, AMOUNT);

				mock_swap(currency_1, &account_1, currency_2, &account_2);

				Ok(())
			})
		}

		pub fn mock_pending_swap_to_local() {
			mock_order_details(USDC_LOCAL, USDC_1);
			mock_fill_order(USDC_1, USER_1, USDC_LOCAL, TokenMux::account());
		}

		pub fn mock_pending_swap_from_local() {
			// swap USDC_1 <-> USDC_LOCAL between USER_1 and TokenMux::account() to fund
			// both accounts sufficiently
			mock_pending_swap_to_local();
			assert_ok!(TokenMux::match_swap(
				RuntimeOrigin::signed(USER_NON),
				ORDER_ID,
				AMOUNT
			));

			// mock opposite direction
			mock_order_details(USDC_1, USDC_LOCAL);
			mock_fill_order(USDC_LOCAL, USER_1, USDC_1, TokenMux::account());
		}

		fn mock_swap(
			currency_1: CurrencyId,
			account_1: &AccountId,
			currency_2: CurrencyId,
			account_2: &AccountId,
		) {
			assert_ok!(<OrmlTokens as Mutate<AccountId>>::transfer(
				currency_1,
				account_1,
				account_2,
				AMOUNT,
				Preservation::Expendable,
			));
			assert_ok!(<OrmlTokens as Mutate<AccountId>>::transfer(
				currency_2,
				account_2,
				account_1,
				AMOUNT,
				Preservation::Expendable,
			));
		}
	}

	#[test]
	fn match_swap_to_local_works() {
		new_test_ext().execute_with(|| {
			mock_pending_swap_to_local();

			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_1), 0);
			assert_eq!(OrmlTokens::free_balance(USDC_1, &TokenMux::account()), 0);

			assert_ok!(TokenMux::match_swap(
				RuntimeOrigin::signed(USER_NON),
				ORDER_ID,
				AMOUNT
			));
			System::assert_last_event(RuntimeEvent::TokenMux(SwapMatched {
				id: ORDER_ID,
				amount: AMOUNT,
			}));

			assert_eq!(
				OrmlTokens::free_balance(USDC_1, &USER_1),
				INITIAL_AMOUNT - AMOUNT
			);
			assert_eq!(
				OrmlTokens::free_balance(USDC_1, &TokenMux::account()),
				AMOUNT
			);
			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_1), AMOUNT);
			assert_eq!(
				OrmlTokens::free_balance(USDC_LOCAL, &TokenMux::account()),
				0
			);
		})
	}

	#[test]
	fn match_swap_from_local_works() {
		new_test_ext().execute_with(|| {
			mock_pending_swap_from_local();

			assert_eq!(
				OrmlTokens::free_balance(USDC_1, &USER_1),
				INITIAL_AMOUNT - AMOUNT
			);
			assert_eq!(
				OrmlTokens::free_balance(USDC_1, &TokenMux::account()),
				AMOUNT
			);
			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_1), AMOUNT);
			assert_eq!(
				OrmlTokens::free_balance(USDC_LOCAL, &TokenMux::account()),
				0
			);

			assert_ok!(TokenMux::match_swap(
				RuntimeOrigin::signed(USER_NON),
				ORDER_ID,
				AMOUNT
			));
			System::assert_last_event(RuntimeEvent::TokenMux(SwapMatched {
				id: ORDER_ID,
				amount: AMOUNT,
			}));

			assert_eq!(OrmlTokens::free_balance(USDC_1, &USER_1), INITIAL_AMOUNT);
			assert_eq!(OrmlTokens::free_balance(USDC_1, &TokenMux::account()), 0);
			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_1), 0);
			assert_eq!(
				OrmlTokens::free_balance(USDC_LOCAL, &TokenMux::account()),
				0
			);
		})
	}

	#[test]
	fn match_swap_from_to_local_throws() {
		new_test_ext().execute_with(|| {
			mock_order_details(USDC_LOCAL, USDC_LOCAL);

			assert_noop!(
				TokenMux::match_swap(RuntimeOrigin::signed(USER_NON), ORDER_ID, AMOUNT),
				Error::<Runtime>::InvalidSwapCurrencies
			);
		})
	}

	#[test]
	fn match_swap_from_to_variant_throws() {
		new_test_ext().execute_with(|| {
			mock_order_details(USDC_1, USDC_2);

			assert_noop!(
				TokenMux::match_swap(RuntimeOrigin::signed(USER_NON), ORDER_ID, AMOUNT),
				Error::<Runtime>::InvalidSwapCurrencies
			);
		})
	}

	#[test]
	fn match_swap_metadata_not_found() {
		new_test_ext().execute_with(|| {
			mock_order_details(UNREGISTERED_ASSET, USDC_LOCAL);

			assert_noop!(
				TokenMux::match_swap(RuntimeOrigin::signed(USER_NON), ORDER_ID, AMOUNT),
				Error::<Runtime>::InvalidSwapCurrencies
			);

			mock_order_details(USDC_LOCAL, UNREGISTERED_ASSET);

			assert_noop!(
				TokenMux::match_swap(RuntimeOrigin::signed(USER_NON), ORDER_ID, AMOUNT),
				Error::<Runtime>::InvalidSwapCurrencies
			);
		})
	}

	#[test]
	fn match_swap_decimal_mismatch() {
		new_test_ext().execute_with(|| {
			mock_order_details(NON_USDC, USDC_LOCAL);

			assert_noop!(
				TokenMux::match_swap(RuntimeOrigin::signed(USER_NON), ORDER_ID, AMOUNT),
				Error::<Runtime>::InvalidSwapCurrencies
			);

			mock_order_details(USDC_LOCAL, NON_USDC);

			assert_noop!(
				TokenMux::match_swap(RuntimeOrigin::signed(USER_NON), ORDER_ID, AMOUNT),
				Error::<Runtime>::InvalidSwapCurrencies
			);
		})
	}
}
