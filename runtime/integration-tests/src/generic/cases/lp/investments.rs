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
use pallet_investments::OrderOf;
use sp_runtime::traits::Zero;

use crate::{
	generic::{
		cases::lp::{names, setup_full, utils::pool_c_tranche_1_id, DECIMALS_6, POOL_C},
		config::Runtime,
		env::{Blocks, Env, EnvEvmExtension},
	},
	utils::accounts::Keyring,
};

const DEFAULT_INVESTMENT_AMOUNT: Balance = 100 * DECIMALS_6;

mod utils {
	use cfg_primitives::AccountId;
	use cfg_traits::investments::TrancheCurrency;
	use cfg_types::domain_address::DomainAddress;
	use ethabi::Token;
	use runtime_common::account_conversion::AccountConverter;
	use sp_core::U256;
	use sp_runtime::traits::Convert;

	use crate::{
		generic::{
			cases::lp::{investments::DEFAULT_INVESTMENT_AMOUNT, EVM_DOMAIN_CHAIN_ID},
			config::Runtime,
			env::EvmEnv,
			utils::{collect_investments, pool::close_epoch},
		},
		utils::accounts::Keyring,
	};

	pub fn account_from_remote(keyring: Keyring) -> AccountId {
		AccountConverter::convert(DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, keyring.into()))
	}

	pub fn investment_id<T: pallet_pool_system::Config>(
		pool: T::PoolId,
		tranche: T::TrancheId,
	) -> <T as pallet_pool_system::Config>::TrancheCurrency {
		<T as pallet_pool_system::Config>::TrancheCurrency::generate(pool, tranche)
	}

	pub fn invest<T: Runtime>(evm: &mut impl EvmEnv<T>, who: Keyring, lp_pool: &str) {
		evm.call(
			who,
			U256::zero(),
			lp_pool,
			"requestDeposit",
			Some(&[
				Token::Uint(DEFAULT_INVESTMENT_AMOUNT.into()),
				Token::Address(who.into()),
				Token::Address(who.into()),
				Token::Bytes(Default::default()),
			]),
		)
		.unwrap();
	}

	pub fn close_and_collect<T: Runtime>(
		investor: Keyring,
		pool: <T as pallet_pool_system::Config>::PoolId,
		tranche: <T as pallet_pool_system::Config>::TrancheId,
	) {
		close_epoch::<T>(Keyring::Admin.id(), pool);
		collect_investments::<T>(investor.id(), pool, tranche);
	}
}

#[test]
fn _test() {
	with_pool_currency_collect::<centrifuge_runtime::Runtime>()
}

fn with_pool_currency_invest<T: Runtime>() {
	let mut env = setup_full::<T>();
	env.state_mut(|evm| {
		utils::invest(evm, Keyring::TrancheInvestor(1), names::POOL_C_T_1_USDC);
	});

	env.state(|_| {
		assert_eq!(
			pallet_investments::InvestOrders::<T>::get(
				utils::account_from_remote(Keyring::TrancheInvestor(1)),
				utils::investment_id::<T>(POOL_C, pool_c_tranche_1_id::<T>())
			),
			Some(OrderOf::<T>::new(
				DEFAULT_INVESTMENT_AMOUNT,
				OrderId::zero()
			))
		)
	});
}

// TODO: CHANGE EVM TO BE ENVIRONMENTAL AND MAKE TRAIT NON SELF BUT RATHER GET
// THAT ENVIRONMENTAL!

fn with_pool_currency_collect<T: Runtime>() {
	let mut env = setup_full::<T>();
	env.state_mut(|evm| {
		utils::invest(evm, Keyring::TrancheInvestor(1), names::POOL_C_T_1_USDC);
	});
	// Needed to get passed min_epoch_time
	env.pass(Blocks::ByNumber(1));
	env.state_mut(|_evm| {
		utils::close_and_collect::<T>(
			Keyring::TrancheInvestor(1),
			POOL_C,
			pool_c_tranche_1_id::<T>(),
		);
	});
	// Needed for processing outbound queue
	env.pass(Blocks::ByNumber(1));
}
