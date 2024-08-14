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

use cfg_primitives::{Balance, OrderId};
use ethabi::{Token, Uint};
use pallet_investments::OrderOf;
use sp_core::U256;
use sp_runtime::traits::Zero;

use crate::{
	cases::lp::{
		self, names, setup_full,
		utils::{pool_c_tranche_1_id, Decoder},
		DECIMALS_6, POOL_C,
	},
	config::Runtime,
	env::{Blocks, Env, EnvEvmExtension, EvmEnv},
	utils::accounts::Keyring,
};

const DEFAULT_INVESTMENT_AMOUNT: Balance = 100 * DECIMALS_6;

mod utils {
	use cfg_primitives::{AccountId, InvestmentId, PoolId, TrancheId};
	use cfg_traits::HasLocalAssetRepresentation;
	use ethabi::Token;
	use pallet_foreign_investments::Action;
	use pallet_liquidity_pools::{GeneralCurrencyIndexOf, GeneralCurrencyIndexType};
	use sp_core::U256;

	use crate::{
		cases::lp::{investments::DEFAULT_INVESTMENT_AMOUNT, names, utils::Decoder},
		config::Runtime,
		env::EvmEnv,
		utils::{accounts::Keyring, collect_investments, pool::close_epoch},
	};

	pub fn index_lp<T: Runtime>(evm: &mut impl EvmEnv<T>, name: &str) -> GeneralCurrencyIndexType {
		Decoder::<GeneralCurrencyIndexType>::decode(&evm.view(
			Keyring::Alice,
			names::POOL_MANAGER,
			"assetToId",
			Some(&[Token::Address(evm.deployed(name).address)]),
		))
	}

	pub fn currency_index<T: Runtime>(
		currency_id: <T as pallet_liquidity_pools::Config>::CurrencyId,
	) -> GeneralCurrencyIndexType {
		GeneralCurrencyIndexOf::<T>::try_from(currency_id)
			.unwrap()
			.index
	}

	pub fn investment_id<T: pallet_pool_system::Config>(
		pool: PoolId,
		tranche: TrancheId,
	) -> InvestmentId {
		(pool, tranche)
	}

	// TODO: CHANGE EVM TO BE ENVIRONMENTAL AND MAKE TRAIT NON SELF BUT RATHER GET
	//       THAT ENVIRONMENTAL!
	pub fn invest<T: Runtime>(evm: &mut impl EvmEnv<T>, who: Keyring, lp_pool: &str) {
		evm.call(
			who,
			U256::zero(),
			lp_pool,
			"requestDeposit",
			Some(&[
				Token::Uint(DEFAULT_INVESTMENT_AMOUNT.into()),
				Token::Address(who.in_eth()),
				Token::Address(who.in_eth()),
			]),
		)
		.unwrap();
	}

	pub fn cancel<T: Runtime>(evm: &mut impl EvmEnv<T>, who: Keyring, lp_pool: &str) {
		evm.call(
			who,
			Default::default(),
			lp_pool,
			"cancelDepositRequest",
			Some(&[Token::Uint(U256::from(0)), Token::Address(who.in_eth())]),
		)
		.unwrap();
	}

	pub fn close_and_collect<T: Runtime>(
		investor: AccountId,
		pool: <T as pallet_pool_system::Config>::PoolId,
		tranche: <T as pallet_pool_system::Config>::TrancheId,
	) {
		close_epoch::<T>(Keyring::Admin.id(), pool);
		// NOTE: We are collecting for the remote accounts only here.
		collect_investments::<T>(investor, pool, tranche);
	}

	pub fn fulfill_swap<T: Runtime>(
		investor: AccountId,
		pool: <T as pallet_pool_system::Config>::PoolId,
		tranche: <T as pallet_pool_system::Config>::TrancheId,
		action: Action,
		amount: Option<<T as pallet_order_book::Config>::BalanceOut>,
	) {
		let order = pallet_order_book::Orders::<T>::get(
			pallet_foreign_investments::Pallet::<T>::order_id(
				&investor,
				investment_id::<T>(pool, tranche),
				action,
			)
			.expect("Nothing to match"),
		)
		.unwrap();

		let from = &order.currency_out;
		let to = &order.currency_in;
		let needs_token_mux = match (
			HasLocalAssetRepresentation::<orml_asset_registry::module::Pallet<T>>::is_local_representation_of(to, from).unwrap(),
			HasLocalAssetRepresentation::<orml_asset_registry::module::Pallet<T>>::is_local_representation_of(from, to).unwrap(),
		) {
			(true, true) => unreachable!("Currencies should never be local of locals."),
			(false, false) => false,
			(true, false) => true,
			(false, true) => true,
		};

		if needs_token_mux {
			pallet_token_mux::Pallet::<T>::match_swap(
				Keyring::Alice.as_origin::<T::RuntimeOriginExt>(),
				order.order_id,
				amount.unwrap_or(order.amount_out),
			)
			.unwrap();
		} else {
			// TODO: Need to move tokens to Centrifuge first IIRC
			//       (i.e. FRAX, DAI, USDC) and then match. Best would be
			//       to move them during start-up, swap some USDC against
			//       LocalUSDC so that Alice holds it all.
		}
	}
}

mod with_pool_currency {
	use super::{utils, *};
	use crate::cases::lp::utils as lp_utils;

	#[test_runtimes([centrifuge, development])]
	fn currency_invest<T: Runtime>() {
		let mut env = setup_full::<T>();
		env.state_mut(|evm| {
			utils::invest(evm, Keyring::TrancheInvestor(1), names::POOL_C_T_1_USDC);
		});

		env.pass(Blocks::ByNumber(1));

		env.state(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_C, pool_c_tranche_1_id::<T>())
				),
				Some(OrderOf::<T>::new(
					DEFAULT_INVESTMENT_AMOUNT,
					OrderId::zero()
				))
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_C_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				DEFAULT_INVESTMENT_AMOUNT
			);
		});
	}

	#[test_runtimes([centrifuge, development])]
	fn currency_collect<T: Runtime>() {
		let mut env = setup_full::<T>();
		env.state_mut(|evm| {
			utils::invest(evm, Keyring::TrancheInvestor(1), names::POOL_C_T_1_USDC);
		});
		// Needed to get passed min_epoch_time
		env.pass(Blocks::ByNumber(1));
		env.state_mut(|_evm| {
			utils::close_and_collect::<T>(
				lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
				POOL_C,
				pool_c_tranche_1_id::<T>(),
			);

			lp_utils::process_gateway_message::<T>(lp_utils::verify_gateway_message_success::<T>);
		});

		env.state_mut(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_C, pool_c_tranche_1_id::<T>())
				),
				None
			);

			evm.call(
				Keyring::TrancheInvestor(1),
				U256::zero(),
				names::POOL_C_T_1_USDC,
				"deposit",
				Some(&[
					Token::Uint(Decoder::<Uint>::decode(&evm.view(
						Keyring::TrancheInvestor(1),
						names::POOL_C_T_1_USDC,
						"maxDeposit",
						Some(&[Token::Address(Keyring::TrancheInvestor(1).in_eth())]),
					))),
					Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					Token::Address(Keyring::TrancheInvestor(1).in_eth()),
				]),
			)
			.unwrap();

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_C_T_1,
					"balanceOf",
					Some(&[Token::Address(Keyring::TrancheInvestor(1).in_eth())]),
				)),
				// Same amount as price is 1.
				DEFAULT_INVESTMENT_AMOUNT
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_C_T_1_USDC,
					"maxDeposit",
					Some(&[Token::Address(Keyring::TrancheInvestor(1).in_eth())]),
				)),
				0
			);
		});
	}

	#[test_runtimes([centrifuge, development])]
	fn invest_cancel_full<T: Runtime>() {
		let mut env = setup_full::<T>();
		env.state_mut(|evm| {
			utils::invest(evm, Keyring::TrancheInvestor(1), names::POOL_C_T_1_USDC);
		});

		env.pass(Blocks::ByNumber(1));

		env.state(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_C, pool_c_tranche_1_id::<T>())
				),
				Some(OrderOf::<T>::new(
					DEFAULT_INVESTMENT_AMOUNT,
					OrderId::zero()
				))
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_C_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				DEFAULT_INVESTMENT_AMOUNT
			);
		});

		env.state_mut(|evm| {
			utils::cancel(evm, Keyring::TrancheInvestor(1), names::POOL_C_T_1_USDC);

			lp_utils::process_gateway_message::<T>(lp_utils::verify_gateway_message_success::<T>);
		});

		env.pass(Blocks::ByNumber(1));

		env.state_mut(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_C, pool_c_tranche_1_id::<T>())
				),
				None
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_C_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				0
			);
		});
	}
}

mod with_foreign_currency {
	use cfg_types::fixed_point::Quantity;
	use cfg_utils::vec_to_fixed_array;
	use pallet_foreign_investments::Action;
	use pallet_liquidity_pools::Message;
	use sp_runtime::{
		traits::{EnsureFixedPointNumber, EnsureSub, One},
		FixedPointNumber,
	};

	use super::{utils, *};
	use crate::cases::lp::{
		investments::utils::close_and_collect,
		utils as lp_utils,
		utils::{as_h160_32bytes, pool_a_tranche_1_id},
		POOL_A,
	};

	#[test_runtimes([centrifuge, development])]
	fn invest_cancel_full_before_swap<T: Runtime>() {
		let mut env = setup_full::<T>();
		env.state_mut(|evm| {
			utils::invest(evm, Keyring::TrancheInvestor(1), names::POOL_A_T_1_USDC);
		});

		env.pass(Blocks::ByNumber(1));

		env.state(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_A, pool_a_tranche_1_id::<T>())
				),
				None,
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				DEFAULT_INVESTMENT_AMOUNT
			);
		});

		env.state_mut(|evm| {
			utils::cancel(evm, Keyring::TrancheInvestor(1), names::POOL_A_T_1_USDC);

			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_A, pool_a_tranche_1_id::<T>())
				),
				None
			);

			lp_utils::process_gateway_message::<T>(lp_utils::verify_gateway_message_success::<T>);
		});

		env.pass(Blocks::ByNumber(1));

		env.state(|evm| {
			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				0
			);
		});
	}

	#[test_runtimes([centrifuge, development])]
	fn invest_cancel_full_after_swap<T: Runtime>() {
		let mut env = setup_full::<T>();

		// Invest and swap all foreign to pool currency
		env.state_mut(|evm| {
			utils::invest(evm, Keyring::TrancheInvestor(1), names::POOL_A_T_1_USDC);
		});

		env.pass(Blocks::ByNumber(1));

		env.state_mut(|_| {
			utils::fulfill_swap::<T>(
				lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
				POOL_A,
				pool_a_tranche_1_id::<T>(),
				Action::Investment,
				// Fulfill entire order
				None,
			);
		});

		env.state(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_A, pool_a_tranche_1_id::<T>())
				),
				Some(OrderOf::<T>::new(
					DEFAULT_INVESTMENT_AMOUNT,
					OrderId::zero()
				))
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				DEFAULT_INVESTMENT_AMOUNT
			);
		});

		env.state_mut(|evm| {
			utils::cancel(evm, Keyring::TrancheInvestor(1), names::POOL_A_T_1_USDC);
		});

		env.pass(Blocks::ByNumber(1));

		env.state_mut(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_A, pool_a_tranche_1_id::<T>())
				),
				None
			);

			utils::fulfill_swap::<T>(
				lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
				POOL_A,
				pool_a_tranche_1_id::<T>(),
				Action::Investment,
				None,
			);

			lp_utils::process_gateway_message::<T>(|msg| {
				assert_eq!(
					msg,
					Message::FulfilledCancelDepositRequest {
						pool_id: POOL_A,
						tranche_id: pool_a_tranche_1_id::<T>(),
						investor: vec_to_fixed_array(lp::utils::remote_account_of::<T>(
							Keyring::TrancheInvestor(1)
						)),
						currency: utils::index_lp(evm, names::USDC),
						currency_payout: DEFAULT_INVESTMENT_AMOUNT,
						fulfilled_invest_amount: DEFAULT_INVESTMENT_AMOUNT,
					},
				)
			});

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				0
			);
		});
	}

	#[test_runtimes([centrifuge, development])]
	fn invest_cancel_full_after_swap_partially<T: Runtime>() {
		let mut env = setup_full::<T>();
		let part = Quantity::checked_from_rational(1, 2).unwrap();
		let partial_amount = part.ensure_mul_int(DEFAULT_INVESTMENT_AMOUNT).unwrap();

		env.state_mut(|evm| {
			utils::invest(evm, Keyring::TrancheInvestor(1), names::POOL_A_T_1_USDC);
		});

		env.pass(Blocks::ByNumber(1));

		env.state_mut(|_| {
			utils::fulfill_swap::<T>(
				lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
				POOL_A,
				pool_a_tranche_1_id::<T>(),
				Action::Investment,
				Some(partial_amount),
			);
		});

		env.state(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_A, pool_a_tranche_1_id::<T>())
				),
				Some(OrderOf::<T>::new(partial_amount, OrderId::zero()))
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				DEFAULT_INVESTMENT_AMOUNT
			);
		});

		env.state_mut(|evm| {
			utils::cancel(evm, Keyring::TrancheInvestor(1), names::POOL_A_T_1_USDC);
		});

		env.pass(Blocks::ByNumber(1));

		env.state_mut(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_A, pool_a_tranche_1_id::<T>())
				),
				None
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				DEFAULT_INVESTMENT_AMOUNT
			);

			utils::fulfill_swap::<T>(
				lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
				POOL_A,
				pool_a_tranche_1_id::<T>(),
				Action::Investment,
				None,
			);

			lp_utils::process_gateway_message::<T>(|msg| {
				assert_eq!(
					msg,
					Message::FulfilledCancelDepositRequest {
						pool_id: POOL_A,
						tranche_id: pool_a_tranche_1_id::<T>(),
						investor: vec_to_fixed_array(lp::utils::remote_account_of::<T>(
							Keyring::TrancheInvestor(1)
						)),
						currency: utils::index_lp(evm, names::USDC),
						currency_payout: DEFAULT_INVESTMENT_AMOUNT,
						fulfilled_invest_amount: DEFAULT_INVESTMENT_AMOUNT,
					},
				)
			});

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				0
			);
		});
	}

	#[test_runtimes([centrifuge, development])]
	fn invest_cancel_full_after_swap_partially_inter_epoch_close<T: Runtime>() {
		let mut env = setup_full::<T>();
		let part = Quantity::checked_from_rational(1, 3).unwrap();
		let other_part = Quantity::one().ensure_sub(part).unwrap();
		let partial_amount = part.ensure_mul_int(DEFAULT_INVESTMENT_AMOUNT).unwrap();
		let remaining_amount = other_part
			.ensure_mul_int(DEFAULT_INVESTMENT_AMOUNT)
			.unwrap();

		env.state_mut(|evm| {
			utils::invest(evm, Keyring::TrancheInvestor(1), names::POOL_A_T_1_USDC);
		});

		env.pass(Blocks::ByNumber(1));

		env.state_mut(|_| {
			utils::fulfill_swap::<T>(
				lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
				POOL_A,
				pool_a_tranche_1_id::<T>(),
				Action::Investment,
				Some(partial_amount),
			);
		});

		env.state(|evm| {
			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_A, pool_a_tranche_1_id::<T>())
				),
				Some(OrderOf::<T>::new(partial_amount, OrderId::zero()))
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				DEFAULT_INVESTMENT_AMOUNT
			);
		});

		env.pass(Blocks::ByNumber(1));

		env.state_mut(|evm| {
			close_and_collect::<T>(
				lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
				POOL_A,
				pool_a_tranche_1_id::<T>(),
			);

			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_A, pool_a_tranche_1_id::<T>())
				),
				None
			);

			lp_utils::process_gateway_message::<T>(|msg| {
				assert_eq!(
					msg,
					Message::FulfilledDepositRequest {
						pool_id: POOL_A,
						tranche_id: pool_a_tranche_1_id::<T>(),
						investor: vec_to_fixed_array(lp::utils::remote_account_of::<T>(
							Keyring::TrancheInvestor(1)
						)),
						currency: utils::index_lp(evm, names::USDC),
						currency_payout: partial_amount,
						tranche_tokens_payout: partial_amount,
					},
				)
			});

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				remaining_amount
			);
		});

		env.state_mut(|evm| {
			utils::cancel(evm, Keyring::TrancheInvestor(1), names::POOL_A_T_1_USDC);

			assert_eq!(
				pallet_investments::InvestOrders::<T>::get(
					lp::utils::remote_account_of::<T>(Keyring::TrancheInvestor(1)),
					utils::investment_id::<T>(POOL_A, pool_a_tranche_1_id::<T>())
				),
				None
			);

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				remaining_amount
			);

			lp_utils::process_gateway_message::<T>(|msg| {
				assert_eq!(
					msg,
					Message::CancelDepositRequest {
						pool_id: POOL_A,
						tranche_id: pool_a_tranche_1_id::<T>(),
						investor: as_h160_32bytes(Keyring::TrancheInvestor(1)),
						currency: utils::index_lp(evm, names::USDC),
					}
				);
			});

			lp_utils::process_gateway_message::<T>(|msg| {
				assert_eq!(
					msg,
					Message::FulfilledCancelDepositRequest {
						pool_id: POOL_A,
						tranche_id: pool_a_tranche_1_id::<T>(),
						investor: vec_to_fixed_array(lp::utils::remote_account_of::<T>(
							Keyring::TrancheInvestor(1)
						)),
						currency: utils::index_lp(evm, names::USDC),
						currency_payout: DEFAULT_INVESTMENT_AMOUNT - partial_amount,
						fulfilled_invest_amount: DEFAULT_INVESTMENT_AMOUNT - partial_amount,
					},
				)
			});

			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::TrancheInvestor(1),
					names::POOL_A_T_1_USDC,
					"pendingDepositRequest",
					Some(&[
						Token::Uint(Uint::zero()),
						Token::Address(Keyring::TrancheInvestor(1).in_eth()),
					]),
				)),
				0
			);
		});
	}
}
