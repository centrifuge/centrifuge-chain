use cfg_primitives::{AccountId, Balance, PoolId};
use cfg_traits::{investments::TrancheCurrency as _, Seconds};
use cfg_types::{
	investments::InvestmentPortfolio,
	permissions::PoolRole,
	tokens::{CurrencyId, TrancheCurrency},
};
use frame_support::traits::fungibles::MutateHold;
use runtime_common::apis::{
	runtime_decl_for_investments_api::InvestmentsApiV1, runtime_decl_for_pools_api::PoolsApiV1,
};
use sp_core::Get;

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::runtime_env::RuntimeEnv,
		utils::{
			self,
			currency::{cfg, usd6, CurrencyInfo, Usd6},
			genesis::{self, Genesis},
			POOL_MIN_EPOCH_TIME,
		},
	},
	utils::accounts::Keyring,
};

const POOL_ADMIN: Keyring = Keyring::Admin;
const INVESTOR: Keyring = Keyring::Alice;
const POOL_A: PoolId = 23;
const EXPECTED_POOL_BALANCE: Balance = usd6(1_000_000);
const REDEEM_AMOUNT: Balance = EXPECTED_POOL_BALANCE / 2;
const HOLD_AMOUNT: Balance = EXPECTED_POOL_BALANCE / 10;
const FOR_FEES: Balance = cfg(1);

mod common {
	use super::*;

	pub fn initialize_state_for_investments<E: Env<T>, T: Runtime>() -> E {
		let mut env = E::from_storage(
			Default::default(),
			Genesis::<T>::default()
				.add(genesis::balances(T::ExistentialDeposit::get() + FOR_FEES))
				.add(genesis::assets(vec![Usd6::ID]))
				.add(genesis::tokens(vec![(Usd6::ID, Usd6::ED)]))
				.storage(),
			Genesis::<T>::default().storage(),
		);

		env.parachain_state_mut(|| {
			// Create a pool
			utils::give_balance::<T>(POOL_ADMIN.id(), T::PoolDeposit::get());
			utils::create_empty_pool::<T>(POOL_ADMIN.id(), POOL_A, Usd6::ID);

			// Grant permissions
			let tranche_id = T::Api::tranche_id(POOL_A, 0).unwrap();
			let tranche_investor = PoolRole::TrancheInvestor(tranche_id, Seconds::MAX);
			utils::give_pool_role::<T>(INVESTOR.id(), POOL_A, tranche_investor);
		});

		env
	}
}

fn investment_portfolio_single_tranche<T: Runtime>() {
	let mut env = common::initialize_state_for_investments::<RuntimeEnv<T>, T>();

	let tranche_id = env.parachain_state(|| T::Api::tranche_id(POOL_A, 0).unwrap());
	let invest_id = TrancheCurrency::generate(POOL_A, tranche_id);

	let mut investment_portfolio =
		env.parachain_state(|| T::Api::investment_portfolio(INVESTOR.id()));
	assert_eq!(investment_portfolio, vec![]);

	// Invest to have pending pool currency
	env.parachain_state_mut(|| {
		utils::give_tokens::<T>(INVESTOR.id(), Usd6::ID, EXPECTED_POOL_BALANCE);
		utils::invest::<T>(INVESTOR.id(), POOL_A, tranche_id, EXPECTED_POOL_BALANCE);
		assert_eq!(
			pallet_investments::InvestOrders::<T>::get(INVESTOR.id(), invest_id)
				.unwrap()
				.amount(),
			EXPECTED_POOL_BALANCE
		);
	});
	investment_portfolio = env.parachain_state_mut(|| T::Api::investment_portfolio(INVESTOR.id()));
	assert_eq!(
		investment_portfolio,
		vec![(
			invest_id,
			InvestmentPortfolio::<Balance, CurrencyId>::new(Usd6::ID)
				.with_pending_invest_currency(EXPECTED_POOL_BALANCE)
		)]
	);

	// Execute epoch to move pending to claimable pool currency
	env.pass(Blocks::BySeconds(POOL_MIN_EPOCH_TIME));
	env.parachain_state_mut(|| {
		utils::close_pool_epoch::<T>(POOL_ADMIN.id(), POOL_A);
	});
	investment_portfolio = env.parachain_state_mut(|| T::Api::investment_portfolio(INVESTOR.id()));
	assert_eq!(
		investment_portfolio,
		vec![(
			invest_id,
			InvestmentPortfolio::<Balance, CurrencyId>::new(Usd6::ID)
				.with_claimable_tranche_tokens(EXPECTED_POOL_BALANCE)
		)]
	);

	// Collect to move claimable pool currency to free tranche tokens
	env.parachain_state_mut(|| {
		utils::collect_investments::<T>(INVESTOR.id(), POOL_A, tranche_id);
	});
	investment_portfolio = env.parachain_state_mut(|| T::Api::investment_portfolio(INVESTOR.id()));
	assert_eq!(
		investment_portfolio,
		vec![(
			invest_id,
			InvestmentPortfolio::<Balance, CurrencyId>::new(Usd6::ID)
				.with_free_tranche_tokens(EXPECTED_POOL_BALANCE)
		)]
	);

	// Redeem to move free tranche tokens to partially pending
	env.parachain_state_mut(|| {
		utils::redeem::<T>(INVESTOR.id(), POOL_A, tranche_id, REDEEM_AMOUNT);
	});
	investment_portfolio = env.parachain_state_mut(|| T::Api::investment_portfolio(INVESTOR.id()));
	assert_eq!(
		investment_portfolio,
		vec![(
			invest_id,
			InvestmentPortfolio::<Balance, CurrencyId>::new(Usd6::ID)
				.with_free_tranche_tokens(EXPECTED_POOL_BALANCE - REDEEM_AMOUNT)
				.with_pending_redeem_tranche_tokens(REDEEM_AMOUNT)
		)]
	);

	// Execute epoch to move pending tranche tokens to claimable pool currency
	env.pass(Blocks::BySeconds(POOL_MIN_EPOCH_TIME));
	env.parachain_state_mut(|| {
		utils::close_pool_epoch::<T>(POOL_ADMIN.id(), POOL_A);
	});
	investment_portfolio = env.parachain_state_mut(|| T::Api::investment_portfolio(INVESTOR.id()));
	assert_eq!(
		investment_portfolio,
		vec![(
			invest_id,
			InvestmentPortfolio::<Balance, CurrencyId>::new(Usd6::ID)
				.with_free_tranche_tokens(EXPECTED_POOL_BALANCE - REDEEM_AMOUNT)
				.with_claimable_currency(REDEEM_AMOUNT)
		)]
	);

	// Collect redemption to clear claimable pool currency
	env.parachain_state_mut(|| {
		utils::collect_redemptions::<T>(INVESTOR.id(), POOL_A, tranche_id);
	});
	investment_portfolio = env.parachain_state_mut(|| T::Api::investment_portfolio(INVESTOR.id()));
	assert_eq!(
		investment_portfolio,
		vec![(
			invest_id,
			InvestmentPortfolio::<Balance, CurrencyId>::new(Usd6::ID)
				.with_free_tranche_tokens(EXPECTED_POOL_BALANCE - REDEEM_AMOUNT)
		)]
	);

	// Simulate holding
	env.parachain_state_mut(|| {
		<pallet_restricted_tokens::Pallet<T> as MutateHold<AccountId>>::hold(
			invest_id.into(),
			&(),
			&INVESTOR.id(),
			HOLD_AMOUNT,
		)
		.unwrap();
	});
	investment_portfolio = env.parachain_state_mut(|| T::Api::investment_portfolio(INVESTOR.id()));
	assert_eq!(
		investment_portfolio,
		vec![(
			invest_id,
			InvestmentPortfolio::<Balance, CurrencyId>::new(Usd6::ID)
				.with_free_tranche_tokens(EXPECTED_POOL_BALANCE - REDEEM_AMOUNT - HOLD_AMOUNT)
				.with_reserved_tranche_tokens(HOLD_AMOUNT)
		)]
	);
}

crate::test_for_runtimes!(all, investment_portfolio_single_tranche);
