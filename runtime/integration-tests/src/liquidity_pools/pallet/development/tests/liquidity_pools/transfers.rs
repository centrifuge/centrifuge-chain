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

use cfg_primitives::{AccountId, Balance, PoolId, TrancheId, CFG};
use cfg_traits::{
	investments::{OrderManager, TrancheCurrency as TrancheCurrencyT},
	liquidity_pools::InboundQueue,
	Permissions as _,
};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	permissions::{PermissionScope, PoolRole, Role},
	tokens::{
		CrossChainTransferability, CurrencyId, CurrencyId::ForeignAsset, CustomMetadata,
		ForeignAssetId,
	},
};
use frame_support::{assert_noop, assert_ok, dispatch::Weight, traits::fungibles::Mutate};
use fudge::primitives::Chain;
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::account_conversion::AccountConverter;
use sp_runtime::{
	traits::{Convert, One, Zero},
	BoundedVec, DispatchError, Storage,
};
use tokio::runtime::Handle;
use xcm::{latest::MultiLocation, VersionedMultiLocation};

use crate::{
	chain::centrifuge::{
		LiquidityPools, LocationToAccountId, OrmlTokens, Permissions, PoolSystem,
		Runtime as DevelopmentRuntime, RuntimeOrigin, System, PARA_ID,
	},
	liquidity_pools::pallet::development::{
		setup::dollar,
		tests::liquidity_pools::setup::{
			asset_metadata, create_ausd_pool, create_currency_pool,
			enable_liquidity_pool_transferability,
			investments::{default_tranche_id, general_currency_index, investment_id},
			liquidity_pools_transferable_multilocation, setup_test_env, LiquidityPoolMessage,
			DEFAULT_BALANCE_GLMR, DEFAULT_DOMAIN_ADDRESS_MOONBEAM, DEFAULT_POOL_ID,
		},
	},
	utils::{accounts::Keyring, env, genesis, AUSD_CURRENCY_ID, AUSD_ED, MOONBEAM_EVM_CHAIN_ID},
};

#[tokio::test]
async fn transfer_non_tranche_tokens_from_local() {
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_native_balances::<DevelopmentRuntime>(&mut genesis);
		env::test_env_with_centrifuge_storage(Handle::current(), genesis)
	};

	setup_test_env(&mut env);

	env.with_mut_state(Chain::Para(PARA_ID), || {
		let initial_balance = 2 * AUSD_ED;
		let amount = initial_balance / 2;
		let dest_address = DEFAULT_DOMAIN_ADDRESS_MOONBEAM;
		let currency_id = AUSD_CURRENCY_ID;
		let source_account = Keyring::Charlie;

		// Mint sufficient balance
		assert_eq!(
			OrmlTokens::free_balance(currency_id, &source_account.into()),
			0
		);
		assert_ok!(OrmlTokens::mint_into(
			currency_id,
			&source_account.into(),
			initial_balance
		));
		assert_eq!(
			OrmlTokens::free_balance(currency_id, &source_account.into()),
			initial_balance
		);

		// Only `ForeignAsset` can be transferred
		assert_noop!(
			LiquidityPools::transfer(
				RuntimeOrigin::signed(source_account.into()),
				CurrencyId::Tranche(42u64, [0u8; 16]),
				dest_address.clone(),
				amount,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidTransferCurrency
		);
		assert_noop!(
			LiquidityPools::transfer(
				RuntimeOrigin::signed(source_account.into()),
				CurrencyId::Staking(cfg_types::tokens::StakingCurrency::BlockRewards),
				dest_address.clone(),
				amount,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotFound
		);
		assert_noop!(
			LiquidityPools::transfer(
				RuntimeOrigin::signed(source_account.into()),
				CurrencyId::Native,
				dest_address.clone(),
				amount,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotFound
		);

		// Cannot transfer as long as cross chain transferability is disabled
		assert_noop!(
			LiquidityPools::transfer(
				RuntimeOrigin::signed(source_account.into()),
				currency_id,
				dest_address.clone(),
				initial_balance,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotLiquidityPoolsTransferable
		);

		// Enable LiquidityPools transferability
		enable_liquidity_pool_transferability(currency_id);

		// Cannot transfer more than owned
		assert_noop!(
			LiquidityPools::transfer(
				RuntimeOrigin::signed(source_account.into()),
				currency_id,
				dest_address.clone(),
				initial_balance.saturating_add(1),
			),
			orml_tokens::Error::<DevelopmentRuntime>::BalanceTooLow
		);

		assert_ok!(LiquidityPools::transfer(
			RuntimeOrigin::signed(source_account.into()),
			currency_id,
			dest_address.clone(),
			amount,
		));

		// The account to which the currency should have been transferred
		// to on Centrifuge for bookkeeping purposes.
		let domain_account: AccountId = Domain::convert(dest_address.domain());
		// Verify that the correct amount of the token was transferred
		// to the dest domain account on Centrifuge.
		assert_eq!(
			OrmlTokens::free_balance(currency_id, &domain_account),
			amount
		);
		assert_eq!(
			OrmlTokens::free_balance(currency_id, &source_account.into()),
			initial_balance - amount
		);
	});
}

#[tokio::test]
async fn transfer_non_tranche_tokens_to_local() {
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_native_balances::<DevelopmentRuntime>(&mut genesis);
		env::test_env_with_centrifuge_storage(Handle::current(), genesis)
	};

	setup_test_env(&mut env);

	env.with_mut_state(Chain::Para(PARA_ID), || {
		let initial_balance = DEFAULT_BALANCE_GLMR;
		let amount = DEFAULT_BALANCE_GLMR / 2;
		let dest_address = DEFAULT_DOMAIN_ADDRESS_MOONBEAM;
		let currency_id = AUSD_CURRENCY_ID;
		let receiver: AccountId = Keyring::Bob.into();

		// Mock incoming decrease message
		let msg = LiquidityPoolMessage::Transfer {
			currency: general_currency_index(currency_id),
			// sender is irrelevant for other -> local
			sender: Keyring::Alice.into(),
			receiver: receiver.clone().into(),
			amount,
		};

		// assert_eq!(OrmlTokens::total_issuance(currency_id), AUSD_ED * 2);
		assert_eq!(OrmlTokens::total_issuance(currency_id), 0);

		// Finally, verify that we can now transfer the tranche to the destination
		// address
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Verify that the correct amount was minted
		// assert_eq!(
		// 	OrmlTokens::total_issuance(currency_id),
		// 	amount + AUSD_ED * 2
		// );
		assert_eq!(OrmlTokens::total_issuance(currency_id), amount);
		// assert_eq!(
		// 	OrmlTokens::free_balance(currency_id, &receiver),
		// 	amount + AUSD_ED
		// );
		assert_eq!(OrmlTokens::free_balance(currency_id, &receiver), amount);

		// Verify empty transfers throw
		assert_noop!(
			LiquidityPools::submit(
				DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				LiquidityPoolMessage::Transfer {
					currency: general_currency_index(currency_id),
					sender: Keyring::Alice.into(),
					receiver: receiver.into(),
					amount: 0,
				},
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidTransferAmount
		);
	});
}

#[tokio::test]
async fn transfer_tranche_tokens_from_local() {
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
		env::test_env_with_centrifuge_storage(Handle::current(), genesis)
	};

	setup_test_env(&mut env);

	env.with_mut_state(Chain::Para(PARA_ID), || {
		let pool_id = DEFAULT_POOL_ID;
		let amount = 100_000;
		let dest_address: DomainAddress = DomainAddress::EVM(1284, [99; 20]);
		let receiver = Keyring::Bob;

		// Create the pool
		create_ausd_pool(pool_id);

		let tranche_tokens: CurrencyId =
			cfg_types::tokens::TrancheCurrency::generate(pool_id, default_tranche_id(pool_id))
				.into();

		// Verify that we first need the destination address to be whitelisted
		assert_noop!(
			LiquidityPools::transfer_tranche_tokens(
				RuntimeOrigin::signed(Keyring::Alice.into()),
				pool_id,
				default_tranche_id(pool_id),
				dest_address.clone(),
				amount,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::UnauthorizedTransfer
		);

		// Make receiver the MembersListAdmin of this Pool
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			receiver.into(),
			PermissionScope::Pool(pool_id),
			Role::PoolRole(PoolRole::InvestorAdmin),
		));

		// Whitelist destination as TrancheInvestor of this Pool
		let valid_until = u64::MAX;
		assert_ok!(Permissions::add(
			RuntimeOrigin::signed(receiver.into()),
			Role::PoolRole(PoolRole::InvestorAdmin),
			AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert(
				dest_address.clone()
			),
			PermissionScope::Pool(pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(
				default_tranche_id(pool_id),
				valid_until
			)),
		));

		// Call the LiquidityPools::update_member which ensures the destination address
		// is whitelisted.
		assert_ok!(LiquidityPools::update_member(
			RuntimeOrigin::signed(receiver.into()),
			pool_id,
			default_tranche_id(pool_id),
			dest_address.clone(),
			valid_until,
		));

		// Give receiver enough Tranche balance to be able to transfer it
		OrmlTokens::deposit(tranche_tokens, &receiver.into(), amount);

		// Finally, verify that we can now transfer the tranche to the destination
		// address
		assert_ok!(LiquidityPools::transfer_tranche_tokens(
			RuntimeOrigin::signed(receiver.into()),
			pool_id,
			default_tranche_id(pool_id),
			dest_address.clone(),
			amount,
		));

		// The account to which the tranche should have been transferred
		// to on Centrifuge for bookkeeping purposes.
		let domain_account: AccountId = Domain::convert(dest_address.domain());

		// Verify that the correct amount of the Tranche token was transferred
		// to the dest domain account on Centrifuge.
		assert_eq!(
			OrmlTokens::free_balance(tranche_tokens, &domain_account),
			amount
		);
		assert!(OrmlTokens::free_balance(tranche_tokens, &receiver.into()).is_zero());
	});
}

#[tokio::test]
async fn transfer_tranche_tokens_to_local() {
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
		env::test_env_with_centrifuge_storage(Handle::current(), genesis)
	};

	setup_test_env(&mut env);

	env.with_mut_state(Chain::Para(PARA_ID), || {
		// Create new pool
		let pool_id = DEFAULT_POOL_ID;
		create_ausd_pool(pool_id);

		let amount = 100_000_000;
		let receiver: AccountId = Keyring::Bob.into();
		let sender: DomainAddress = DomainAddress::EVM(1284, [99; 20]);
		let sending_domain_locator = Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
		let tranche_id = default_tranche_id(pool_id);
		let tranche_tokens: CurrencyId =
			cfg_types::tokens::TrancheCurrency::generate(pool_id, tranche_id).into();
		let valid_until = u64::MAX;

		// Fund `DomainLocator` account of origination domain tranche tokens are
		// transferred from this account instead of minting
		assert_ok!(OrmlTokens::mint_into(
			tranche_tokens,
			&sending_domain_locator,
			amount
		));

		// Mock incoming decrease message
		let msg = LiquidityPoolMessage::TransferTrancheTokens {
			pool_id,
			tranche_id,
			sender: sender.address(),
			domain: Domain::Centrifuge,
			receiver: receiver.clone().into(),
			amount,
		};

		// Verify that we first need the receiver to be whitelisted
		assert_noop!(
			LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::UnauthorizedTransfer
		);

		// Make receiver the MembersListAdmin of this Pool
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			receiver.clone(),
			PermissionScope::Pool(pool_id),
			Role::PoolRole(PoolRole::InvestorAdmin),
		));

		// Whitelist destination as TrancheInvestor of this Pool
		assert_ok!(Permissions::add(
			RuntimeOrigin::signed(receiver.clone()),
			Role::PoolRole(PoolRole::InvestorAdmin),
			receiver.clone(),
			PermissionScope::Pool(pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(
				default_tranche_id(pool_id),
				valid_until
			)),
		));

		// Finally, verify that we can now transfer the tranche to the destination
		// address
		assert_ok!(LiquidityPools::submit(DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg));

		// Verify that the correct amount of the Tranche token was transferred
		// to the dest domain account on Centrifuge.
		assert_eq!(OrmlTokens::free_balance(tranche_tokens, &receiver), amount);
		assert!(OrmlTokens::free_balance(tranche_tokens, &sending_domain_locator).is_zero());
	});
}

/// Try to transfer tranches for non-existing pools or invalid tranche ids for
/// existing pools.
#[tokio::test]
async fn transferring_invalid_tranche_tokens_should_fail() {
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_balances::<DevelopmentRuntime>(&mut genesis);
		env::test_env_with_centrifuge_storage(Handle::current(), genesis)
	};

	setup_test_env(&mut env);

	env.with_mut_state(Chain::Para(PARA_ID), || {
		let dest_address: DomainAddress = DomainAddress::EVM(1284, [99; 20]);

		let valid_pool_id: u64 = 42;
		create_ausd_pool(valid_pool_id);
		let valid_tranche_id = default_tranche_id(valid_pool_id);
		let valid_until = u64::MAX;
		let transfer_amount = 42;
		let invalid_pool_id = valid_pool_id + 1;
		let invalid_tranche_id = valid_tranche_id.map(|i| i.saturating_add(1));
		assert!(PoolSystem::pool(invalid_pool_id).is_none());

		// Make Keyring::Bob the MembersListAdmin of both pools
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			Keyring::Bob.into(),
			PermissionScope::Pool(valid_pool_id),
			Role::PoolRole(PoolRole::InvestorAdmin),
		));
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			Keyring::Bob.into(),
			PermissionScope::Pool(invalid_pool_id),
			Role::PoolRole(PoolRole::InvestorAdmin),
		));

		// Give Keyring::Bob investor role for (valid_pool_id, invalid_tranche_id) and
		// (invalid_pool_id, valid_tranche_id)
		assert_ok!(Permissions::add(
			RuntimeOrigin::signed(Keyring::Bob.into()),
			Role::PoolRole(PoolRole::InvestorAdmin),
			AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert(
				dest_address.clone()
			),
			PermissionScope::Pool(invalid_pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(valid_tranche_id, valid_until)),
		));
		assert_ok!(Permissions::add(
			RuntimeOrigin::signed(Keyring::Bob.into()),
			Role::PoolRole(PoolRole::InvestorAdmin),
			AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert(
				dest_address.clone()
			),
			PermissionScope::Pool(valid_pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(invalid_tranche_id, valid_until)),
		));
		assert_noop!(
			LiquidityPools::transfer_tranche_tokens(
				RuntimeOrigin::signed(Keyring::Bob.into()),
				invalid_pool_id,
				valid_tranche_id,
				dest_address.clone(),
				transfer_amount
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::PoolNotFound
		);
		assert_noop!(
			LiquidityPools::transfer_tranche_tokens(
				RuntimeOrigin::signed(Keyring::Bob.into()),
				valid_pool_id,
				invalid_tranche_id,
				dest_address,
				transfer_amount
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::TrancheNotFound
		);
	});
}
