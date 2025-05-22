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

use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;

use crate::{
	mock::{
		new_test_ext, token, token_with, Balance, CurrencyId, MockTokenSwaps, OrmlTokens, Runtime,
		RuntimeEvent, RuntimeOrigin, SwapId, System, TokenMux, DIFF_DEC_USDC_DECIMALS,
		INITIAL_AMOUNT, NON_USDC, UNREGISTERED_ASSET, USDC_1, USDC_2, USDC_LOCAL, USER_1, USER_2,
		USER_LOCAL, USER_NON, USER_UNREGISTERED,
	},
	Error,
	Event::{Burned, Deposited},
};

pub const ORDER_ID: SwapId = 1;

pub const AMOUNT: Balance = token(1000);

pub const AMOUNT_DIFF_DEC_USDC: Balance = token_with(1000, DIFF_DEC_USDC_DECIMALS);

mod deposit {
	use super::*;
	use crate::{
		mock::{INITIAL_DIFF_DEC_USDC_AMOUNT, USDC_DIFF_DECIMALS, USER_3},
		tests::swaps::utils::{
			mock_fill_order, mock_fill_order_with, mock_order_details, mock_order_details_with,
			mock_place_order, mock_place_order_with,
		},
	};

	#[test]
	fn deposit_usdc_variant_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(OrmlTokens::free_balance(USDC_1, &USER_1), INITIAL_AMOUNT);
			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_1), 0);
			assert_eq!(
				OrmlTokens::free_balance(USDC_LOCAL, &TokenMux::account()),
				0
			);

			mock_order_details(USDC_LOCAL, USDC_1);
			mock_place_order(USER_1, USDC_LOCAL, USDC_1);
			mock_fill_order(USDC_LOCAL, TokenMux::account(), USDC_1, USER_1);

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
	fn deposit_usdc_diff_decimals_variant_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				OrmlTokens::free_balance(USDC_DIFF_DECIMALS, &USER_3),
				INITIAL_DIFF_DEC_USDC_AMOUNT
			);
			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_3), 0);
			assert_eq!(
				OrmlTokens::free_balance(USDC_LOCAL, &TokenMux::account()),
				0
			);

			mock_place_order_with(USER_3, USDC_LOCAL, USDC_DIFF_DECIMALS, AMOUNT_DIFF_DEC_USDC);
			mock_order_details_with(USDC_LOCAL, USDC_DIFF_DECIMALS, AMOUNT_DIFF_DEC_USDC);
			mock_fill_order_with(
				USDC_DIFF_DECIMALS,
				USER_3,
				USDC_LOCAL,
				TokenMux::account(),
				AMOUNT_DIFF_DEC_USDC,
				AMOUNT,
			);

			assert_ok!(TokenMux::deposit(
				RuntimeOrigin::signed(USER_3),
				USDC_DIFF_DECIMALS,
				AMOUNT_DIFF_DEC_USDC
			));

			assert_eq!(
				OrmlTokens::free_balance(USDC_DIFF_DECIMALS, &USER_3),
				INITIAL_DIFF_DEC_USDC_AMOUNT - AMOUNT_DIFF_DEC_USDC
			);
			assert_eq!(OrmlTokens::free_balance(USDC_LOCAL, &USER_3), AMOUNT);
			assert_eq!(
				OrmlTokens::free_balance(USDC_DIFF_DECIMALS, &TokenMux::account()),
				AMOUNT_DIFF_DEC_USDC
			);

			System::assert_last_event(RuntimeEvent::TokenMux(Deposited {
				who: USER_3,
				currency_out: USDC_DIFF_DECIMALS,
				currency_in: USDC_LOCAL,
				amount: AMOUNT_DIFF_DEC_USDC,
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
			mock_order_details(USDC_LOCAL, USDC_1);
			mock_place_order(USER_1, USDC_LOCAL, USDC_1);
			mock_fill_order(USDC_LOCAL, TokenMux::account(), USDC_1, USER_1);

			assert_ok!(TokenMux::deposit(
				RuntimeOrigin::signed(USER_1),
				USDC_1,
				AMOUNT
			));

			mock_order_details(USDC_LOCAL, USDC_2);
			mock_place_order(USER_2, USDC_LOCAL, USDC_2);
			mock_fill_order(USDC_LOCAL, TokenMux::account(), USDC_2, USER_2);

			assert_ok!(TokenMux::deposit(
				RuntimeOrigin::signed(USER_2),
				USDC_2,
				AMOUNT
			));

			mock_order_details(USDC_2, USDC_LOCAL);
			mock_place_order(USER_1, USDC_2, USDC_LOCAL);
			mock_fill_order(USDC_2, TokenMux::account(), USDC_LOCAL, USER_1);

			assert_ok!(TokenMux::burn(
				RuntimeOrigin::signed(USER_1),
				USDC_2,
				AMOUNT
			));

			mock_order_details(USDC_1, USDC_LOCAL);
			mock_place_order(USER_2, USDC_1, USDC_LOCAL);
			mock_fill_order(USDC_1, TokenMux::account(), USDC_LOCAL, USER_2);

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
			mock_order_details(USDC_LOCAL, USDC_2);
			mock_place_order(USER_1, USDC_LOCAL, USDC_2);
			mock_fill_order(USDC_LOCAL, TokenMux::account(), USDC_2, USER_1);

			assert_noop!(
				TokenMux::deposit(RuntimeOrigin::signed(USER_1), USDC_2, AMOUNT),
				orml_tokens::Error::<Runtime>::BalanceTooLow
			);
		})
	}
}

mod burn {
	use cfg_traits::swaps::{OrderInfo, OrderRatio, Swap};
	use frame_support::traits::{fungibles::Mutate, tokens::Preservation};
	use sp_runtime::{traits::One, DispatchError};

	use super::*;
	use crate::{
		mock::{AccountId, Ratio},
		tests::swaps::utils::{mock_fill_order, mock_order_details, mock_place_order},
	};

	#[test]
	fn burn_usdc_variant_works() {
		new_test_ext().execute_with(|| {
			mock_order_details(USDC_LOCAL, USDC_1);
			mock_place_order(USER_1, USDC_LOCAL, USDC_1);
			mock_fill_order(USDC_LOCAL, TokenMux::account(), USDC_1, USER_1);

			assert_ok!(TokenMux::deposit(
				RuntimeOrigin::signed(USER_1),
				USDC_1,
				AMOUNT
			));

			MockTokenSwaps::mock_get_order_details(move |order_id| {
				assert_eq!(order_id, ORDER_ID);
				Some(OrderInfo {
					swap: Swap {
						currency_in: USDC_1,
						currency_out: USDC_LOCAL,
						amount_out: AMOUNT / 2,
					},
					ratio: OrderRatio::Custom(Ratio::one()),
				})
			});

			MockTokenSwaps::mock_place_order(
				move |who, currency_in, currency_out, amount, ratio| {
					assert_eq!(who, USER_1);
					assert_eq!(currency_in, USDC_1);
					assert_eq!(currency_out, USDC_LOCAL);
					assert_eq!(amount, AMOUNT / 2);
					assert_eq!(ratio, OrderRatio::Custom(Ratio::one()));

					Ok::<SwapId, DispatchError>(ORDER_ID)
				},
			);

			MockTokenSwaps::mock_fill_order_no_slip_prot(move |_who, order_id, amount_out| {
				assert_eq!(order_id, ORDER_ID);
				assert_eq!(amount_out, AMOUNT / 2);

				assert_ok!(<OrmlTokens as Mutate<AccountId>>::transfer(
					USDC_LOCAL,
					&USER_1,
					&TokenMux::account(),
					AMOUNT / 2,
					Preservation::Expendable,
				));
				assert_ok!(<OrmlTokens as Mutate<AccountId>>::transfer(
					USDC_1,
					&TokenMux::account(),
					&USER_1,
					AMOUNT / 2,
					Preservation::Expendable,
				));

				Ok(())
			});

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
			mock_place_order(USER_1, USDC_1, USDC_LOCAL);
			mock_order_details(USDC_1, USDC_LOCAL);
			mock_fill_order(USDC_1, USER_1, USDC_LOCAL, TokenMux::account());

			assert_noop!(
				TokenMux::burn(RuntimeOrigin::signed(USER_1), USDC_1, AMOUNT),
				orml_tokens::Error::<Runtime>::BalanceTooLow
			);
		})
	}
}

pub(crate) mod try_local {
	use super::*;
	use crate::mock::{new_test_ext_invalid_assets, HAS_UNREGISTERED_LOCAL_ASSET};

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
}

pub(crate) mod swaps {
	use super::*;
	use crate::{
		tests::swaps::utils::{
			mock_order_details, mock_pending_swap_from_local, mock_pending_swap_to_local,
		},
		Event::SwapMatched,
	};

	pub(crate) mod utils {
		use cfg_traits::swaps::{OrderInfo, OrderRatio, Swap};
		use frame_support::traits::tokens::{fungibles::Mutate, Preservation};
		use sp_arithmetic::traits::One;
		use sp_runtime::{DispatchError, DispatchResult};

		use super::*;
		use crate::mock::{AccountId, Ratio};

		pub fn mock_order_details(currency_in: CurrencyId, currency_out: CurrencyId) {
			mock_order_details_with(currency_in, currency_out, AMOUNT)
		}

		pub fn mock_order_details_with(
			currency_in: CurrencyId,
			currency_out: CurrencyId,
			_amount: Balance,
		) {
			MockTokenSwaps::mock_get_order_details(move |order_id| {
				assert_eq!(order_id, ORDER_ID);
				Some(OrderInfo {
					swap: Swap {
						currency_in,
						currency_out,
						amount_out: _amount,
					},
					ratio: OrderRatio::Custom(Ratio::one()),
				})
			});
		}

		pub fn mock_place_order(
			_who: AccountId,
			_currency_in: CurrencyId,
			_currency_out: CurrencyId,
		) {
			mock_place_order_with(_who, _currency_in, _currency_out, AMOUNT)
		}

		pub fn mock_place_order_with(
			_who: AccountId,
			_currency_in: CurrencyId,
			_currency_out: CurrencyId,
			_amount: Balance,
		) {
			MockTokenSwaps::mock_place_order(
				move |who, currency_in, currency_out, amount, ratio| {
					assert_eq!(who, _who);
					assert_eq!(currency_in, _currency_in);
					assert_eq!(currency_out, _currency_out);
					assert_eq!(amount, _amount);
					assert_eq!(ratio, OrderRatio::Custom(Ratio::one()));

					Ok::<SwapId, DispatchError>(ORDER_ID)
				},
			);
		}

		pub fn mock_fill_order(
			currency_1: CurrencyId,
			account_1: AccountId,
			currency_2: CurrencyId,
			account_2: AccountId,
		) {
			mock_fill_order_with(currency_1, account_1, currency_2, account_2, AMOUNT, AMOUNT)
		}

		pub fn mock_fill_order_with(
			currency_1: CurrencyId,
			account_1: AccountId,
			currency_2: CurrencyId,
			account_2: AccountId,
			_amount_out: Balance,
			_amount_in: Balance,
		) {
			MockTokenSwaps::mock_fill_order_no_slip_prot(move |_who, order_id, amount_out| {
				assert_eq!(order_id, ORDER_ID);
				assert_eq!(amount_out, _amount_out);

				mock_swap(
					currency_1,
					&account_1,
					currency_2,
					&account_2,
					_amount_out,
					_amount_in,
				)?;

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

		pub fn mock_swap(
			currency_1: CurrencyId,
			account_1: &AccountId,
			currency_2: CurrencyId,
			account_2: &AccountId,
			amount_out_of_1: Balance,
			amount_out_of_2: Balance,
		) -> DispatchResult {
			<OrmlTokens as Mutate<AccountId>>::transfer(
				currency_1,
				account_1,
				account_2,
				amount_out_of_1,
				Preservation::Expendable,
			)?;
			<OrmlTokens as Mutate<AccountId>>::transfer(
				currency_2,
				account_2,
				account_1,
				amount_out_of_2,
				Preservation::Expendable,
			)
			.map(|_| ())
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

mod is_local_representation {
	use cfg_traits::HasLocalAssetRepresentation;
	use cfg_types::tokens::CurrencyId;
	use sp_runtime::DispatchError;

	use super::*;
	use crate::mock::{new_test_ext_invalid_assets, MockRegistry, USDC_DIFF_DECIMALS};

	#[test]
	fn is_local_happy_paths() {
		new_test_ext().execute_with(|| {
            assert_eq!(<CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(&USDC_LOCAL, &USDC_1), Ok(true));
            assert_eq!(
                <CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(
                    &USDC_1,
                    &USDC_LOCAL
                ),
                Ok(false)
            );
            assert_eq!(<CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(&USDC_LOCAL, &USDC_2), Ok(true));
            assert_eq!(
                <CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(
                    &USDC_2,
                    &USDC_LOCAL
                ),
                Ok(false)
            );

            assert_eq!(<CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(&USDC_LOCAL, &NON_USDC), Ok(false));
            assert_eq!(
                <CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(
                    &NON_USDC,
                    &USDC_LOCAL
                ),
                Ok(false)
            );
        });
	}

	#[test]
	fn is_local_missing_cannot_lookup() {
		new_test_ext().execute_with(|| {
            assert_eq!(<CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(&UNREGISTERED_ASSET, &USDC_1), Err(DispatchError::CannotLookup));
            assert_eq!(<CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(&USDC_1, &UNREGISTERED_ASSET), Err(DispatchError::CannotLookup));

            assert_eq!(<CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(&USDC_LOCAL, &UNREGISTERED_ASSET), Err(DispatchError::CannotLookup));
            assert_eq!(<CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(&UNREGISTERED_ASSET, &USDC_LOCAL), Err(DispatchError::CannotLookup));
        });
	}
	#[test]
	fn is_local_diff_decimals() {
		new_test_ext_invalid_assets().execute_with(|| {
            assert_eq!(<CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(&USDC_LOCAL, &USDC_DIFF_DECIMALS),Ok(true));
            assert_eq!(<CurrencyId as HasLocalAssetRepresentation<MockRegistry>>::is_local_representation_of(&USDC_DIFF_DECIMALS, &USDC_LOCAL), Ok(false));
        });
	}
}
