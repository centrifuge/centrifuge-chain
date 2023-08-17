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

use ::xcm::{
	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId},
	prelude::{Parachain, X1, X2},
	VersionedMultiLocation,
};
use cfg_primitives::{currency_decimals, parachains, AccountId, Balance, PoolId, TrancheId, CFG};
use cfg_traits::{
	liquidity_pools::{Codec, InboundQueue},
	OrderManager, Permissions as _, PoolMutate, TrancheCurrency,
};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Rate,
	investments::InvestmentAccount,
	orders::FulfillmentWithPrice,
	permissions::{PermissionScope, PoolRole, Role, UNION},
	pools::TrancheMetadata,
	tokens::{
		CrossChainTransferability, CurrencyId, CurrencyId::ForeignAsset, CustomMetadata,
		ForeignAssetId, LiquidityPoolsWrappedToken,
	},
	xcm::XcmMetadata,
};
use codec::Encode;
use development_runtime::{
	Balances, Investments, LiquidityPools, LiquidityPoolsGateway, LiquidityPoolsPalletId, Loans,
	OrmlAssetRegistry, OrmlTokens, Permissions, PoolSystem, Runtime as DevelopmentRuntime,
	RuntimeOrigin, System, TreasuryAccount, XTokens, XcmTransactor,
};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::Weight,
	traits::{fungibles::Mutate, Get, PalletInfo},
};
use hex::FromHex;
use liquidity_pools_gateway_routers::XcmDomain as GatewayXcmDomain;
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use pallet_liquidity_pools::{
	encoded_contract_call, Error::UnauthorizedTransfer, Message, ParachainId, Router, XcmDomain,
};
use pallet_pool_system::{
	pool_types::PoolDetails,
	tranches::{TrancheInput, TrancheLoc, TrancheType},
};
use runtime_common::{
	account_conversion::AccountConverter, xcm::general_key, xcm_fees::default_per_second,
};
use sp_core::H160;
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, ConstU32, Convert, EnsureAdd, One, Zero},
	BoundedVec, DispatchError, Perquintill, SaturatedConversion, WeakBoundedVec,
};
use utils::investments::{
	default_tranche_id, general_currency_index, investment_account, investment_id,
};
use xcm_emulator::TestExt;

use crate::{
	liquidity_pools::pallet::development::{
		setup::{cfg, dollar, ALICE, BOB, PARA_ID_MOONBEAM},
		test_net::{Development, Moonbeam, RelayChain, TestNet},
		tests::liquidity_pools::utils::{
			get_default_moonbeam_native_token_location, DEFAULT_BALANCE_GLMR,
			DEFAULT_MOONBEAM_LOCATION,
		},
	},
	utils::{AUSD_CURRENCY_ID, GLIMMER_CURRENCY_ID, MOONBEAM_EVM_CHAIN_ID},
	*,
};

/// NOTE: We can't actually verify that the messages hits the
/// LiquidityPoolsXcmRouter contract on Moonbeam since that would require a
/// rather heavy e2e setup to emulate, involving depending on Moonbeam's
/// runtime, having said contract deployed to their evm environment, and be able
/// to query the evm side. Instead, these tests verify that - given all
/// pre-requirements are set up correctly - we succeed to send the message from
/// the Centrifuge chain pov. We have other unit tests verifying the
/// LiquidityPools' messages encoding and the encoding of the remote EVM call to
/// be executed on Moonbeam.

/// Verify that `LiquidityPools::add_pool` succeeds when called with all the
/// necessary requirements.
#[test]
fn add_pool() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();
		let pool_id: u64 = 42;

		// Verify that the pool must exist before we can call LiquidityPools::add_pool
		assert_noop!(
			LiquidityPools::add_pool(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				Domain::EVM(1284),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::PoolNotFound
		);

		// Now create the pool
		utils::create_ausd_pool(pool_id);

		// Verify ALICE can't call `add_pool` given she is not the `PoolAdmin`
		assert_noop!(
			LiquidityPools::add_pool(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				Domain::EVM(1284),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::NotPoolAdmin
		);

		// Verify that it works if it's BOB calling it (the pool admin)
		assert_ok!(LiquidityPools::add_pool(
			RuntimeOrigin::signed(BOB.into()),
			pool_id,
			Domain::EVM(1284),
		));
	});
}

/// Verify that `LiquidityPools::add_tranche` succeeds when called with all the
/// necessary requirements. We can't actually verify that the call hits the
/// LiquidityPoolsXcmRouter contract on Moonbeam since that would require a very
/// heavy e2e setup to emulate. Instead, here we test that we can send the
/// extrinsic and we have other unit tests verifying the encoding of the remote
/// EVM call to be executed on Moonbeam.
#[test]
fn add_tranche() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();
		let decimals: u8 = 15;

		// Now create the pool
		let pool_id: u64 = 42;
		utils::create_ausd_pool(pool_id);

		// Verify we can't call LiquidityPools::add_tranche with a non-existing
		// tranche_id
		let nonexistent_tranche = [71u8; 16];
		assert_noop!(
			LiquidityPools::add_tranche(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				nonexistent_tranche,
				Domain::EVM(1284),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::TrancheNotFound
		);

		// Find the right tranche id
		let pool_details = PoolSystem::pool(pool_id).expect("Pool should exist");
		let tranche_id = pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists");

		// Verify ALICE can't call `add_tranche` given she is not the `PoolAdmin`
		assert_noop!(
			LiquidityPools::add_tranche(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				tranche_id,
				Domain::EVM(1284),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::NotPoolAdmin
		);

		// Finally, verify we can call LiquidityPools::add_tranche successfully
		// when called by the PoolAdmin with the right pool + tranche id pair.
		assert_ok!(LiquidityPools::add_tranche(
			RuntimeOrigin::signed(BOB.into()),
			pool_id,
			tranche_id,
			Domain::EVM(1284),
		));
	});
}

#[test]
fn update_member() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		// Now create the pool
		let pool_id: u64 = 42;
		utils::create_ausd_pool(pool_id);

		// Find the right tranche id
		let pool_details = PoolSystem::pool(pool_id).expect("Pool should exist");
		let tranche_id = pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists");

		// Finally, verify we can call LiquidityPools::add_tranche successfully
		// when given a valid pool + tranche id pair.
		let new_member = DomainAddress::EVM(1284, [3; 20]);
		let valid_until = utils::DEFAULT_VALIDITY;

		// Make ALICE the MembersListAdmin of this Pool
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			ALICE.into(),
			PermissionScope::Pool(pool_id),
			Role::PoolRole(PoolRole::InvestorAdmin),
		));

		// Verify it fails if the destination is not whitelisted yet
		assert_noop!(
			LiquidityPools::update_member(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				tranche_id,
				new_member.clone(),
				valid_until,
			),
			pallet_liquidity_pools::Error::<development_runtime::Runtime>::InvestorDomainAddressNotAMember,
		);

		// Whitelist destination as TrancheInvestor of this Pool
		assert_ok!(Permissions::add(
			RuntimeOrigin::signed(ALICE.into()),
			Role::PoolRole(PoolRole::InvestorAdmin),
			AccountConverter::<DevelopmentRuntime>::convert(new_member.clone()),
			PermissionScope::Pool(pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(
				default_tranche_id(pool_id),
				valid_until
			)),
		));

		// Verify the Investor role was set as expected in Permissions
		assert!(Permissions::has(
			PermissionScope::Pool(pool_id),
			AccountConverter::<DevelopmentRuntime>::convert(new_member.clone()),
			Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, valid_until)),
		));

		// Verify it now works
		assert_ok!(LiquidityPools::update_member(
			RuntimeOrigin::signed(ALICE.into()),
			pool_id,
			tranche_id,
			new_member,
			valid_until,
		));

		// Verify it cannot be called for another member without whitelisting the domain
		// beforehand
		assert_noop!(
			LiquidityPools::update_member(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				tranche_id,
				DomainAddress::EVM(1284, [9; 20]),
				valid_until,
			),
			pallet_liquidity_pools::Error::<development_runtime::Runtime>::InvestorDomainAddressNotAMember,
		);
	});
}

#[test]
fn update_token_price() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();
		let decimals: u8 = 15;

		// Now create the pool
		let pool_id: u64 = 42;
		utils::create_ausd_pool(pool_id);

		// Find the right tranche id
		let pool_details = PoolSystem::pool(pool_id).expect("Pool should exist");
		let tranche_id = pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists");

		// Verify it now works
		assert_ok!(LiquidityPools::update_token_price(
			RuntimeOrigin::signed(ALICE.into()),
			pool_id,
			tranche_id,
			Domain::EVM(1284),
		));
	});
}

#[test]
fn transfer_non_tranche_tokens_from_local() {
	TestNet::reset();

	Development::execute_with(|| {
		// Register GLMR and fund BOB
		utils::setup_pre_requirements();

		let initial_balance = 100_000_000;
		let amount = initial_balance / 2;
		let dest_address = utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM;
		let currency_id = AUSD_CURRENCY_ID;
		let source_account = BOB;

		// Mint sufficient balance
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
		assert_ok!(OrmlAssetRegistry::update_asset(
			RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			// Changed: Add location which can be converted to LiquidityPoolsWrappedToken
			Some(Some(utils::liquidity_pools_transferable_multilocation(
				MOONBEAM_EVM_CHAIN_ID,
				// Value of evm_address is irrelevant here
				[1u8; 20],
			))),
			Some(CustomMetadata {
				// Changed: Allow liquidity_pools transferability
				transferability: CrossChainTransferability::LiquidityPools,
				mintable: Default::default(),
				permissioned: Default::default(),
				pool_currency: Default::default()
			})
		));

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

#[test]
fn transfer_non_tranche_tokens_to_local() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		let initial_balance = utils::DEFAULT_BALANCE_GLMR;
		let amount = utils::DEFAULT_BALANCE_GLMR / 2;
		let dest_address = utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM;
		let currency_id = AUSD_CURRENCY_ID;
		let receiver: AccountId = BOB.into();

		// Mock incoming decrease message
		let msg = utils::LiquidityPoolMessage::Transfer {
			currency: general_currency_index(currency_id),
			// sender is irrelevant for other -> local
			sender: ALICE,
			receiver: receiver.clone().into(),
			amount,
		};

		assert!(OrmlTokens::total_issuance(currency_id).is_zero());

		// TODO(after PR #1376): Re-activate via Gateway handling
		// // Verify that we do not accept incoming messages if the connection has not
		// been // initialized
		// assert_noop!(
		// 	LiquidityPools::submit(utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
		// 	pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidIncomingMessageOrigin
		// );
		// assert_ok!(LiquidityPools::add_instance(
		// 	RuntimeOrigin::root(),
		// 	utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM.address().into()
		// ));

		// Finally, verify that we can now transfer the tranche to the destination
		// address
		assert_ok!(LiquidityPools::submit(
			utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
			msg
		));

		// Verify that the correct amount was minted
		assert_eq!(OrmlTokens::total_issuance(currency_id), amount);
		assert_eq!(OrmlTokens::free_balance(currency_id, &receiver), amount);

		// Verify empty transfers throw
		assert_noop!(
			LiquidityPools::submit(
				utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				utils::LiquidityPoolMessage::Transfer {
					currency: general_currency_index(currency_id),
					sender: ALICE,
					receiver: receiver.into(),
					amount: 0,
				},
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidTransferAmount
		);
	});
}

#[test]
fn transfer_tranche_tokens_from_local() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		let pool_id: u64 = 42;
		let amount = 100_000;
		let dest_address: DomainAddress = DomainAddress::EVM(1284, [99; 20]);
		let receiver = BOB;

		// Create the pool
		utils::create_ausd_pool(pool_id);

		let tranche_tokens: CurrencyId =
			cfg_types::tokens::TrancheCurrency::generate(pool_id, default_tranche_id(pool_id))
				.into();

		// Verify that we first need the destination address to be whitelisted
		assert_noop!(
			LiquidityPools::transfer_tranche_tokens(
				RuntimeOrigin::signed(ALICE.into()),
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
			AccountConverter::<DevelopmentRuntime>::convert(dest_address.clone()),
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

#[test]
fn transfer_tranche_tokens_to_local() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		// Create new pool
		let pool_id: u64 = 42;
		utils::create_ausd_pool(pool_id);

		let amount = 100_000_000;
		let receiver: AccountId = BOB.into();
		let sender: DomainAddress = DomainAddress::EVM(1284, [99; 20]);
		let sending_domain_locator =
			Domain::convert(utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain());
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
		let msg = utils::LiquidityPoolMessage::TransferTrancheTokens {
			pool_id,
			tranche_id,
			sender: sender.address(),
			domain: Domain::Centrifuge,
			receiver: receiver.clone().into(),
			amount,
		};

		// TODO(after PR #1376): Re-activate via Gateway handling
		// // Verify that we do not accept incoming messages if the connection has not
		// been // initialized
		// assert_noop!(
		// 	LiquidityPools::submit(utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
		// 	pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidIncomingMessageOrigin
		// );
		// assert_ok!(LiquidityPools::add_instance(
		// 	RuntimeOrigin::root(),
		// 	utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM.address().into()
		// ));

		// Verify that we first need the receiver to be whitelisted
		assert_noop!(
			LiquidityPools::submit(utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
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
		assert_ok!(LiquidityPools::submit(
			utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
			msg
		));

		// Verify that the correct amount of the Tranche token was transferred
		// to the dest domain account on Centrifuge.
		assert_eq!(OrmlTokens::free_balance(tranche_tokens, &receiver), amount);
		assert!(OrmlTokens::free_balance(tranche_tokens, &sending_domain_locator).is_zero());

		// TODO(subsequent PR): Verify that we cannot transfer to the local
		// domain blocked by https://github.com/centrifuge/centrifuge-chain/pull/1376
		// assert_noop!(
		// 	LiquidityPools::submit(utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg),
		// 	pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidDomain
		// );
	});
}

#[test]
/// Try to transfer tranches for non-existing pools or invalid tranche ids for
/// existing pools.
fn transferring_invalid_tranche_tokens_should_fail() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();
		let dest_address: DomainAddress = DomainAddress::EVM(1284, [99; 20]);

		let valid_pool_id: u64 = 42;
		utils::create_ausd_pool(valid_pool_id);
		let pool_details = PoolSystem::pool(valid_pool_id).expect("Pool should exist");
		let valid_tranche_id = pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists");
		let valid_until = u64::MAX;
		let transfer_amount = 42;
		let invalid_pool_id = valid_pool_id + 1;
		let invalid_tranche_id = valid_tranche_id.map(|i| i.saturating_add(1));
		assert!(PoolSystem::pool(invalid_pool_id).is_none());

		// Make BOB the MembersListAdmin of both pools
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			BOB.into(),
			PermissionScope::Pool(valid_pool_id),
			Role::PoolRole(PoolRole::InvestorAdmin),
		));
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			BOB.into(),
			PermissionScope::Pool(invalid_pool_id),
			Role::PoolRole(PoolRole::InvestorAdmin),
		));

		// Give BOB investor role for (valid_pool_id, invalid_tranche_id) and
		// (invalid_pool_id, valid_tranche_id)
		assert_ok!(Permissions::add(
			RuntimeOrigin::signed(BOB.into()),
			Role::PoolRole(PoolRole::InvestorAdmin),
			AccountConverter::<DevelopmentRuntime>::convert(dest_address.clone()),
			PermissionScope::Pool(invalid_pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(valid_tranche_id, valid_until)),
		));
		assert_ok!(Permissions::add(
			RuntimeOrigin::signed(BOB.into()),
			Role::PoolRole(PoolRole::InvestorAdmin),
			AccountConverter::<DevelopmentRuntime>::convert(dest_address.clone()),
			PermissionScope::Pool(valid_pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(invalid_tranche_id, valid_until)),
		));
		assert_noop!(
			LiquidityPools::transfer_tranche_tokens(
				RuntimeOrigin::signed(BOB.into()),
				invalid_pool_id,
				valid_tranche_id,
				dest_address.clone(),
				transfer_amount
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::PoolNotFound
		);
		assert_noop!(
			LiquidityPools::transfer_tranche_tokens(
				RuntimeOrigin::signed(BOB.into()),
				valid_pool_id,
				invalid_tranche_id,
				dest_address,
				transfer_amount
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::TrancheNotFound
		);
	});
}

#[test]
fn add_currency() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		let currency_id = AUSD_CURRENCY_ID;
		let location = utils::liquidity_pools_transferable_multilocation(
			MOONBEAM_EVM_CHAIN_ID,
			utils::DEFAULT_EVM_ADDRESS_MOONBEAM,
		);

		// Enable LiquidityPools transferability
		assert_ok!(OrmlAssetRegistry::update_asset(
			RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			// Changed: Add location which can be converted to LiquidityPoolsWrappedToken
			Some(Some(location)),
			Some(CustomMetadata {
				// Changed: Allow liquidity_pools transferability
				transferability: CrossChainTransferability::LiquidityPools,
				mintable: Default::default(),
				permissioned: Default::default(),
				pool_currency: Default::default()
			})
		));

		assert_ok!(LiquidityPools::add_currency(
			RuntimeOrigin::signed(BOB.into()),
			currency_id
		));
	});
}

#[test]
fn add_currency_should_fail() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		assert_noop!(
			LiquidityPools::add_currency(
				RuntimeOrigin::signed(BOB.into()),
				CurrencyId::ForeignAsset(42)
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotFound
		);
		assert_noop!(
			LiquidityPools::add_currency(RuntimeOrigin::signed(BOB.into()), CurrencyId::Native),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotFound
		);
		assert_noop!(
			LiquidityPools::add_currency(
				RuntimeOrigin::signed(BOB.into()),
				CurrencyId::Staking(cfg_types::tokens::StakingCurrency::BlockRewards)
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotFound
		);
		assert_noop!(
			LiquidityPools::add_currency(
				RuntimeOrigin::signed(BOB.into()),
				CurrencyId::Staking(cfg_types::tokens::StakingCurrency::BlockRewards)
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotFound
		);

		// Should fail to add currency_id which is missing a registered
		// MultiLocation
		let currency_id = CurrencyId::ForeignAsset(100);
		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			utils::asset_metadata(
				"Test".into(),
				"TEST".into(),
				12,
				false,
				None,
				CrossChainTransferability::LiquidityPools,
			),
			Some(currency_id)
		));
		assert_noop!(
			LiquidityPools::add_currency(RuntimeOrigin::signed(BOB.into()), currency_id),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotLiquidityPoolsWrappedToken
		);

		// Add convertable MultiLocation to metadata but remove transferability
		assert_ok!(OrmlAssetRegistry::update_asset(
			RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			// Changed: Add multilocation to metadata for some random EVM chain id for which no
			// instance is registered
			Some(Some(utils::liquidity_pools_transferable_multilocation(
				u64::MAX,
				[1u8; 20],
			))),
			Some(CustomMetadata {
				// Changed: Disallow liquidityPools transferability
				transferability: CrossChainTransferability::Xcm(Default::default()),
				mintable: Default::default(),
				permissioned: Default::default(),
				pool_currency: Default::default(),
			}),
		));
		assert_noop!(
			LiquidityPools::add_currency(RuntimeOrigin::signed(BOB.into()), currency_id),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotLiquidityPoolsTransferable
		);

		// Switch transferability from XCM to None
		assert_ok!(OrmlAssetRegistry::update_asset(
			RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			None,
			Some(CustomMetadata {
				// Changed: Disallow cross chain transferability entirely
				transferability: CrossChainTransferability::None,
				mintable: Default::default(),
				permissioned: Default::default(),
				pool_currency: Default::default(),
			})
		));
		assert_noop!(
			LiquidityPools::add_currency(RuntimeOrigin::signed(BOB.into()), currency_id),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotLiquidityPoolsTransferable
		);

		// TODO(subsequent PR): Reactivate later
		// Blocked by https://github.com/centrifuge/centrifuge-chain/pull/1376

		// // Should fail if no domain router is registered for the asset's
		// // metadata evm chain id
		// assert_ok!(OrmlAssetRegistry::update_asset(
		// 	RuntimeOrigin::root(),
		// 	currency_id,
		// 	None,
		// 	None,
		// 	None,
		// 	None,
		// 	None,
		// 	Some(CustomMetadata {
		// 		// Changed: Enable all cross chain transferability in metadata
		// 		transferability: CrossChainTransferability::All(XcmMetadata {
		// 			fee_per_second: Default::default()
		// 		}),
		// 		mintable: Default::default(),
		// 		permissioned: Default::default(),
		// 		pool_currency: Default::default(),
		// 	})
		// ));
		// assert_noop!(
		// 	LiquidityPools::add_currency(RuntimeOrigin::signed(BOB.into()),
		// currency_id),
		// 	pallet_liquidity_pools::Error::<DevelopmentRuntime>::MissingRouter
		// );
	});
}

#[test]
fn allow_pool_currency() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		let currency_id = AUSD_CURRENCY_ID;
		let pool_id: u64 = 42;
		let evm_chain_id: u64 = MOONBEAM_EVM_CHAIN_ID;
		let evm_address = [1u8; 20];

		// Create an AUSD pool
		utils::create_ausd_pool(pool_id);

		// Enable LiquidityPools transferability
		assert_ok!(OrmlAssetRegistry::update_asset(
			RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			// Changed: Add location which can be converted to LiquidityPoolsWrappedToken
			Some(Some(utils::liquidity_pools_transferable_multilocation(
				evm_chain_id,
				evm_address,
			))),
			Some(CustomMetadata {
				// Changed: Allow liquidity_pools transferability
				transferability: CrossChainTransferability::LiquidityPools,
				mintable: Default::default(),
				permissioned: Default::default(),
				pool_currency: true,
			})
		));

		assert_ok!(LiquidityPools::allow_pool_currency(
			RuntimeOrigin::signed(BOB.into()),
			pool_id,
			default_tranche_id(pool_id),
			currency_id,
		));
	});
}

#[test]
fn allow_pool_should_fail() {
	TestNet::reset();

	Development::execute_with(|| {
		let pool_id: u64 = 42;
		let currency_id = CurrencyId::ForeignAsset(42);
		let ausd_currency_id = AUSD_CURRENCY_ID;

		utils::setup_pre_requirements();
		// Should fail if pool does not exist
		assert_noop!(
			LiquidityPools::allow_pool_currency(
				RuntimeOrigin::signed(BOB.into()),
				pool_id,
				// Tranche id is arbitrary in this case as pool does not exist
				[0u8; 16],
				currency_id,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::PoolNotFound
		);

		// Register currency_id with pool_currency set to true
		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			utils::asset_metadata(
				"Test".into(),
				"TEST".into(),
				12,
				true,
				None,
				Default::default(),
			),
			Some(currency_id)
		));

		// Create pool
		utils::create_currency_pool(pool_id, currency_id, 10_000 * dollar(12));

		// Should fail if asset is not pool currency
		assert!(currency_id != ausd_currency_id);
		assert_noop!(
			LiquidityPools::allow_pool_currency(
				RuntimeOrigin::signed(BOB.into()),
				pool_id,
				default_tranche_id(pool_id),
				ausd_currency_id,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidInvestCurrency
		);

		// Should fail if currency is not liquidityPools transferable
		assert_ok!(OrmlAssetRegistry::update_asset(
			RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			None,
			Some(CustomMetadata {
				// Disallow any cross chain transferability
				transferability: CrossChainTransferability::None,
				mintable: Default::default(),
				permissioned: Default::default(),
				// Changed: Allow to be usable as pool currency
				pool_currency: true,
			}),
		));
		assert_noop!(
			LiquidityPools::allow_pool_currency(
				RuntimeOrigin::signed(BOB.into()),
				pool_id,
				default_tranche_id(pool_id),
				currency_id,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotLiquidityPoolsTransferable
		);

		// Should fail if currency does not have any MultiLocation in metadata
		assert_ok!(OrmlAssetRegistry::update_asset(
			RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			None,
			Some(CustomMetadata {
				// Changed: Allow liquidityPools transferability
				transferability: CrossChainTransferability::LiquidityPools,
				mintable: Default::default(),
				permissioned: Default::default(),
				// Still allow to be pool currency
				pool_currency: true,
			}),
		));
		assert_noop!(
			LiquidityPools::allow_pool_currency(
				RuntimeOrigin::signed(BOB.into()),
				pool_id,
				default_tranche_id(pool_id),
				currency_id,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotLiquidityPoolsWrappedToken
		);

		// Should fail if currency does not have LiquidityPoolsWrappedToken location in
		// metadata
		assert_ok!(OrmlAssetRegistry::update_asset(
			RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			// Changed: Add some location which cannot be converted to LiquidityPoolsWrappedToken
			Some(Some(VersionedMultiLocation::V3(Default::default()))),
			// No change for transferability required as it is already allowed for LiquidityPools
			None,
		));
		assert_noop!(
			LiquidityPools::allow_pool_currency(
				RuntimeOrigin::signed(BOB.into()),
				pool_id,
				default_tranche_id(pool_id),
				currency_id,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::AssetNotLiquidityPoolsWrappedToken
		);

		// Create new pool for non foreign asset
		// NOTE: Can be removed after merging https://github.com/centrifuge/centrifuge-chain/pull/1343
		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			utils::asset_metadata(
				"Acala Dollar".into(),
				"AUSD".into(),
				12,
				true,
				None,
				Default::default()
			),
			Some(CurrencyId::AUSD)
		));
		utils::create_currency_pool(pool_id + 1, CurrencyId::AUSD, 10_000 * dollar(12));
		// Should fail if currency is not foreign asset
		assert_noop!(
			LiquidityPools::allow_pool_currency(
				RuntimeOrigin::signed(BOB.into()),
				pool_id + 1,
				// Tranche id is arbitrary in this case, so we don't need to check for the exact
				// pool_id
				default_tranche_id(pool_id + 1),
				CurrencyId::AUSD,
			),
			DispatchError::Token(sp_runtime::TokenError::Unsupported)
		);
	});
}

#[test]
fn schedule_upgrade() {
	use frame_support::traits::fungible::Mutate;

	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		// Only Root can call `schedule_upgrade`
		assert_noop!(
			LiquidityPools::schedule_upgrade(
				RuntimeOrigin::signed(BOB.into()),
				DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [7; 20])
			),
			BadOrigin
		);

		// Failing because Root (?) or the LiquidityPools pallet account has no funds
		assert_noop!(
			LiquidityPools::schedule_upgrade(
				RuntimeOrigin::root(),
				DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [7; 20])
			),
			pallet_xcm_transactor::Error::<DevelopmentRuntime>::UnableToWithdrawAsset
		);

		// The LiquidityPools pallet account needs GLRM to cover the fees of sending
		// this message
		OrmlTokens::deposit(
			GLIMMER_CURRENCY_ID,
			&LiquidityPools::account(),
			DEFAULT_BALANCE_GLMR,
		);

		// Now it finally works
		assert_ok!(LiquidityPools::schedule_upgrade(
			RuntimeOrigin::root(),
			DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [7; 20])
		));
	});
}

#[test]
fn inbound_increase_invest_order() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		let pool_id = 42;
		let amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;

		// Create new pool
		utils::create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial investment
		utils::investments::do_initial_increase_investment(pool_id, amount, investor, currency_id);

		// Verify the order was updated to the amount
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_invest_order(
				investment_id(pool_id, default_tranche_id(pool_id))
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
		utils::setup_pre_requirements();

		let pool_id = 42;
		let invest_amount = 100_000_000;
		let decrease_amount = invest_amount / 3;
		let final_amount = invest_amount - decrease_amount;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;

		// Create new pool
		utils::create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial investment
		utils::investments::do_initial_increase_investment(
			pool_id,
			invest_amount,
			investor.clone(),
			currency_id,
		);

		// Mock incoming decrease message
		let msg = utils::LiquidityPoolMessage::DecreaseInvestOrder {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
			amount: decrease_amount,
		};

		// Execute byte message
		assert_ok!(LiquidityPools::submit(
			utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
			msg
		));

		// Verify investment was decreased into investment account
		assert_eq!(
			OrmlTokens::free_balance(
				currency_id,
				&investment_account(investment_id(pool_id, default_tranche_id(pool_id)))
			),
			final_amount
		);
		// The transfer does not happen right away, so should still be in investor's
		// wallet
		assert_eq!(
			OrmlTokens::free_balance(currency_id, &investor),
			decrease_amount
		);
		assert!(System::events().iter().any(|e| e.event
			== pallet_investments::Event::<DevelopmentRuntime>::InvestOrderUpdated {
				investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
				submitted_at: 0,
				who: investor.clone(),
				amount: final_amount
			}
			.into()));
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_invest_order(
				investment_id(pool_id, default_tranche_id(pool_id))
			)
			.amount,
			final_amount
		);
	});
}

#[test]
fn inbound_collect_invest_order() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		let pool_id = 42;
		let amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;

		// Create new pool
		utils::create_currency_pool(pool_id, currency_id, currency_decimals.into());

		let investment_currency_id: CurrencyId =
			investment_id(pool_id, default_tranche_id(pool_id)).into();

		// Set permissions and execute initial investment
		utils::investments::do_initial_increase_investment(
			pool_id,
			amount,
			investor.clone(),
			currency_id,
		);
		let events_before_collect = System::events();

		// Process and fulfill order
		// NOTE: Without this step, the order id is not cleared and
		// `Event::InvestCollectedForNonClearedOrderId` be dispatched
		assert_ok!(Investments::process_invest_orders(investment_id(
			pool_id,
			default_tranche_id(pool_id)
		)));

		// Tranche tokens will be minted upon fulfillment
		assert_eq!(OrmlTokens::total_issuance(investment_currency_id), 0);
		assert_ok!(Investments::invest_fulfillment(
			investment_id(pool_id, default_tranche_id(pool_id)),
			FulfillmentWithPrice::<Rate> {
				of_amount: Perquintill::one(),
				price: Rate::one(),
			}
		));
		assert_eq!(OrmlTokens::total_issuance(investment_currency_id), amount);

		// Mock collection message msg
		let msg = utils::LiquidityPoolMessage::CollectInvest {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
		};

		// Execute byte message
		assert_ok!(LiquidityPools::submit(
			utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
			msg
		));

		// Remove events before collect execution
		let events_since_collect: Vec<_> = System::events()
			.into_iter()
			.filter(|e| !events_before_collect.contains(e))
			.collect();

		// Verify investment was collected into investor
		assert_eq!(
			OrmlTokens::free_balance(
				investment_id(pool_id, default_tranche_id(pool_id)).into(),
				&investor
			),
			amount
		);

		// Order should have been cleared by fulfilling investment
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_invest_order(
				investment_id(pool_id, default_tranche_id(pool_id))
			)
			.amount,
			0
		);
		assert!(!events_since_collect.iter().any(|e| {
			e.event
			== pallet_investments::Event::<DevelopmentRuntime>::InvestCollectedForNonClearedOrderId {
				investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
				who: investor.clone(),
			}
			.into()
		}));

		// Order should not have been updated since everything is collected
		assert!(!events_since_collect.iter().any(|e| {
			e.event
				== pallet_investments::Event::<DevelopmentRuntime>::InvestOrderUpdated {
					investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
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
					investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
					processed_orders: vec![0],
					who: investor.clone(),
					collection: pallet_investments::InvestCollection::<Balance> {
						payout_investment_invest: amount,
						remaining_investment_invest: 0,
					},
					outcome: pallet_investments::CollectOutcome::FullyCollected,
				}
				.into()
		}));
	});
}

#[test]
fn inbound_increase_redeem_order() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		let pool_id = 42;
		let amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;

		// Create new pool
		utils::create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial redemption
		utils::investments::do_initial_increase_redemption(pool_id, amount, investor, currency_id);

		// Verify amount was noted in the corresponding order
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_redeem_order(
				investment_id(pool_id, default_tranche_id(pool_id))
			)
			.amount,
			amount
		);
	});
}

#[test]
fn inbound_decrease_redeem_order() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		let pool_id = 42;
		let redeem_amount = 100_000_000;
		let decrease_amount = redeem_amount / 3;
		let final_amount = redeem_amount - decrease_amount;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;

		// Create new pool
		utils::create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial redemption
		utils::investments::do_initial_increase_redemption(
			pool_id,
			redeem_amount,
			investor.clone(),
			currency_id,
		);

		// Verify the corresponding redemption order id is 0
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::invest_order_id(investment_id(
				pool_id,
				default_tranche_id(pool_id)
			)),
			0
		);

		// Mock incoming decrease message
		let msg = utils::LiquidityPoolMessage::DecreaseRedeemOrder {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
			currency: general_currency_index(currency_id),
			amount: decrease_amount,
		};

		// Execute byte message
		assert_ok!(LiquidityPools::submit(
			utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
			msg
		));

		// Verify investment was decreased into investment account
		assert_eq!(
			OrmlTokens::free_balance(
				investment_id(pool_id, default_tranche_id(pool_id)).into(),
				&investment_account(investment_id(pool_id, default_tranche_id(pool_id)))
			),
			final_amount
		);
		// Burning does not happen right away, so should still be in investor's wallet
		assert_eq!(
			OrmlTokens::free_balance(
				investment_id(pool_id, default_tranche_id(pool_id)).into(),
				&investor
			),
			decrease_amount
		);

		// Order should have been updated
		assert!(System::events().iter().any(|e| e.event
			== pallet_investments::Event::<DevelopmentRuntime>::RedeemOrderUpdated {
				investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
				submitted_at: 0,
				who: investor.clone(),
				amount: final_amount
			}
			.into()));
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_redeem_order(
				investment_id(pool_id, default_tranche_id(pool_id)),
			)
			.amount,
			final_amount
		);
	});
}

#[test]
fn inbound_collect_redeem_order() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		let pool_id = 42;
		let amount = 100_000_000;
		let investor: AccountId = BOB.into();
		let currency_id = AUSD_CURRENCY_ID;
		let currency_decimals = currency_decimals::AUSD;
		let pool_account =
			pallet_pool_system::pool_types::PoolLocator { pool_id }.into_account_truncating();

		// Create new pool
		utils::create_currency_pool(pool_id, currency_id, currency_decimals.into());

		// Set permissions and execute initial investment
		utils::investments::do_initial_increase_redemption(
			pool_id,
			amount,
			investor.clone(),
			currency_id,
		);
		let events_before_collect = System::events();

		// Fund the pool account with sufficient pool currency, else redemption cannot
		// swap tranche tokens against pool currency
		assert_ok!(OrmlTokens::mint_into(currency_id, &pool_account, amount));

		// Process and fulfill order
		// NOTE: Without this step, the order id is not cleared and
		// `Event::RedeemCollectedForNonClearedOrderId` be dispatched
		assert_ok!(Investments::process_redeem_orders(investment_id(
			pool_id,
			default_tranche_id(pool_id)
		)));
		assert_ok!(Investments::redeem_fulfillment(
			investment_id(pool_id, default_tranche_id(pool_id)),
			FulfillmentWithPrice::<Rate> {
				of_amount: Perquintill::one(),
				price: Rate::one(),
			}
		));

		// Mock collection message msg
		let msg = utils::LiquidityPoolMessage::CollectRedeem {
			pool_id,
			tranche_id: default_tranche_id(pool_id),
			investor: investor.clone().into(),
		};

		// Execute byte message
		assert_ok!(LiquidityPools::submit(
			utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
			msg
		));

		// Remove events before collect execution
		let events_since_collect: Vec<_> = System::events()
			.into_iter()
			.filter(|e| !events_before_collect.contains(e))
			.collect();

		// Verify investment was collected into investor
		assert_eq!(OrmlTokens::free_balance(currency_id, &investor), amount);

		// Order should have been cleared by fulfilling redemption
		assert_eq!(
			pallet_investments::Pallet::<DevelopmentRuntime>::acc_active_redeem_order(
				investment_id(pool_id, default_tranche_id(pool_id))
			)
			.amount,
			0
		);
		assert!(!events_since_collect.iter().any(|e| {
			e.event
			== pallet_investments::Event::<DevelopmentRuntime>::RedeemCollectedForNonClearedOrderId {
				investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
				who: investor.clone(),
			}
			.into()
		}));

		// Order should not have been updated since everything is collected
		assert!(!events_since_collect.iter().any(|e| {
			e.event
				== pallet_investments::Event::<DevelopmentRuntime>::RedeemOrderUpdated {
					investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
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
					investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
					processed_orders: vec![0],
					who: investor.clone(),
					collection: pallet_investments::RedeemCollection::<Balance> {
						payout_investment_redeem: amount,
						remaining_investment_redeem: 0,
					},
					outcome: pallet_investments::CollectOutcome::FullyCollected,
				}
				.into()
		}));
	});
}

#[test]
fn test_vec_to_fixed_array() {
	let src = "TrNcH".as_bytes().to_vec();
	let symbol: [u8; 32] = cfg_utils::vec_to_fixed_array(src);

	assert!(symbol.starts_with("TrNcH".as_bytes()));

	assert_eq!(
		symbol,
		[
			84, 114, 78, 99, 72, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0
		]
	);
}

// Verify that the max tranche token symbol and name lengths are what the
// LiquidityPools pallet expects.
#[test]
fn verify_tranche_fields_sizes() {
	assert_eq!(
		cfg_types::consts::pools::MaxTrancheNameLengthBytes::get(),
		pallet_liquidity_pools::TOKEN_NAME_SIZE as u32
	);
	assert_eq!(
		cfg_types::consts::pools::MaxTrancheSymbolLengthBytes::get(),
		pallet_liquidity_pools::TOKEN_SYMBOL_SIZE as u32
	);
}

mod utils {
	use cfg_primitives::Moment;
	use cfg_types::tokens::CrossChainTransferability;
	use liquidity_pools_gateway_routers::{
		ethereum_xcm::EthereumXCMRouter, DomainRouter, XCMRouter, XcmTransactInfo,
	};

	use super::*;
	use crate::{
		liquidity_pools::pallet::development::tests::register_ausd,
		utils::{AUSD_CURRENCY_ID, GLIMMER_CURRENCY_ID, MOONBEAM_EVM_CHAIN_ID},
	};

	pub const DEFAULT_BALANCE_GLMR: Balance = 10_000_000_000_000_000_000;
	pub const DOMAIN_MOONBEAM: Domain = Domain::EVM(MOONBEAM_EVM_CHAIN_ID);
	pub const DEFAULT_EVM_ADDRESS_MOONBEAM: [u8; 20] = [99; 20];
	pub const DEFAULT_DOMAIN_ADDRESS_MOONBEAM: DomainAddress =
		DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, DEFAULT_EVM_ADDRESS_MOONBEAM);
	pub const DEFAULT_VALIDITY: Moment = 2555583502;
	pub const DEFAULT_OTHER_DOMAIN_ADDRESS: DomainAddress =
		DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [0; 20]);

	pub const DEFAULT_MOONBEAM_LOCATION: MultiLocation = MultiLocation {
		parents: 1,
		interior: X1(Parachain(PARA_ID_MOONBEAM)),
	};
	pub type LiquidityPoolMessage = Message<Domain, PoolId, TrancheId, Balance, Rate>;

	pub fn get_default_moonbeam_native_token_location() -> MultiLocation {
		MultiLocation {
			parents: 1,
			interior: X2(Parachain(PARA_ID_MOONBEAM), general_key(&[0, 1])),
		}
	}

	/// Returns a `VersionedMultiLocation` that can be converted into
	/// `LiquidityPoolsWrappedToken` which is required for cross chain asset
	/// registration and transfer.
	pub fn liquidity_pools_transferable_multilocation(
		chain_id: u64,
		address: [u8; 20],
	) -> VersionedMultiLocation {
		VersionedMultiLocation::V3(MultiLocation {
			parents: 0,
			interior:
				X3(
					PalletInstance(
						<DevelopmentRuntime as frame_system::Config>::PalletInfo::index::<
							LiquidityPools,
						>()
						.expect("LiquidityPools should have pallet index")
						.saturated_into(),
					),
					GlobalConsensus(NetworkId::Ethereum { chain_id }),
					AccountKey20 {
						network: None,
						key: address,
					},
				),
		})
	}

	pub fn set_test_domain_router(
		evm_chain_id: u64,
		xcm_domain_location: VersionedMultiLocation,
		currency_id: CurrencyId,
		fee_location: VersionedMultiLocation,
	) {
		let ethereum_xcm_router = EthereumXCMRouter::<DevelopmentRuntime> {
			router: XCMRouter {
				xcm_domain: GatewayXcmDomain {
					location: Box::new(xcm_domain_location),
					ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
					contract_address: H160::from(utils::DEFAULT_EVM_ADDRESS_MOONBEAM),
					max_gas_limit: 700_000,
					transact_info: XcmTransactInfo {
						transact_extra_weight: 1.into(),
						max_weight: 8_000_000_000_000_000.into(),
						transact_extra_weight_signed: Some(3.into()),
					},
					fee_currency: currency_id,
					fee_per_second: default_per_second(18),
					fee_asset_location: Box::new(fee_location),
				},
				_marker: Default::default(),
			},
			_marker: Default::default(),
		};

		let domain_router = DomainRouter::EthereumXCM(ethereum_xcm_router);
		let domain = Domain::EVM(evm_chain_id);

		assert_ok!(LiquidityPoolsGateway::set_domain_router(
			RuntimeOrigin::root(),
			domain,
			domain_router,
		));
	}

	/// Initializes universally required storage for liquidityPools tests:
	///  * Set the EthereumXCM router which in turn sets:
	///  	* transact info and domain router for Moonbeam `MultiLocation`,
	///  	* fee for GLMR (`GLIMMER_CURRENCY_ID`),
	///  * Register GLMR and AUSD in `OrmlAssetRegistry`,
	///  * Mint 10 GLMR (`DEFAULT_BALANCE_GLMR`) for Alice and Bob.
	///
	/// NOTE: AUSD is the default pool currency in `create_pool`.
	/// Neither AUSD nor GLMR are registered as a liquidityPools-transferable
	/// currency!
	pub fn setup_pre_requirements() {
		/// Set the EthereumXCM router necessary for Moonbeam.
		set_test_domain_router(
			MOONBEAM_EVM_CHAIN_ID,
			DEFAULT_MOONBEAM_LOCATION.into(),
			GLIMMER_CURRENCY_ID,
			get_default_moonbeam_native_token_location().into(),
		);

		/// Register Moonbeam's native token
		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			utils::asset_metadata(
				"Glimmer".into(),
				"GLMR".into(),
				18,
				false,
				Some(VersionedMultiLocation::V3(
					get_default_moonbeam_native_token_location()
				)),
				CrossChainTransferability::Xcm(Default::default()),
			),
			Some(GLIMMER_CURRENCY_ID)
		));

		// Give Alice and BOB enough glimmer to pay for fees
		OrmlTokens::deposit(GLIMMER_CURRENCY_ID, &ALICE.into(), DEFAULT_BALANCE_GLMR);
		OrmlTokens::deposit(GLIMMER_CURRENCY_ID, &BOB.into(), DEFAULT_BALANCE_GLMR);

		// Register AUSD in the asset registry which is the default pool currency in
		// `create_pool`
		register_ausd();
	}

	/// Creates a new pool for the given id with
	///  * BOB as admin and depositor
	///  * Two tranches
	///  * AUSD as pool currency with max reserve 10k.
	pub fn create_ausd_pool(pool_id: u64) {
		create_currency_pool(pool_id, AUSD_CURRENCY_ID, dollar(currency_decimals::AUSD))
	}

	/// Creates a new pool for for the given id with the provided currency.
	///  * BOB as admin and depositor
	///  * Two tranches
	///  * The given `currency` as pool currency with of `currency_decimals`.
	pub fn create_currency_pool(pool_id: u64, currency_id: CurrencyId, currency_decimals: Balance) {
		assert_ok!(PoolSystem::create(
			BOB.into(),
			BOB.into(),
			pool_id,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						// NOTE: For now, we have to set these metadata fields of the first tranche
						// to be convertible to the 32-byte size expected by the liquidity pools
						// AddTranche message.
						token_name: BoundedVec::<
							u8,
							cfg_types::consts::pools::MaxTrancheNameLengthBytes,
						>::try_from("A highly advanced tranche".as_bytes().to_vec(),)
						.expect(""),
						token_symbol: BoundedVec::<
							u8,
							cfg_types::consts::pools::MaxTrancheSymbolLengthBytes,
						>::try_from("TrNcH".as_bytes().to_vec())
						.expect(""),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: One::one(),
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				}
			],
			currency_id,
			currency_decimals,
		));
	}

	/// Returns metadata for the given data with existential deposit of
	/// 1_000_000.
	pub fn asset_metadata(
		name: Vec<u8>,
		symbol: Vec<u8>,
		decimals: u32,
		is_pool_currency: bool,
		location: Option<VersionedMultiLocation>,
		transferability: CrossChainTransferability,
	) -> AssetMetadata<Balance, CustomMetadata> {
		AssetMetadata {
			name,
			symbol,
			decimals,
			location,
			existential_deposit: 1_000_000,
			additional: CustomMetadata {
				transferability,
				mintable: false,
				permissioned: false,
				pool_currency: is_pool_currency,
			},
		}
	}

	pub mod investments {
		use super::*;

		/// Returns the investment account of the given investment_id.
		pub fn investment_account(investment_id: cfg_types::tokens::TrancheCurrency) -> AccountId {
			InvestmentAccount { investment_id }.into_account_truncating()
		}

		pub fn default_investment_account(pool_id: u64) -> AccountId {
			InvestmentAccount {
				investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
			}
			.into_account_truncating()
		}

		/// Returns the investment_id of the given pool and tranche ids.
		pub fn investment_id(
			pool_id: u64,
			tranche_id: TrancheId,
		) -> cfg_types::tokens::TrancheCurrency {
			<DevelopmentRuntime as pallet_liquidity_pools::Config>::TrancheCurrency::generate(
				pool_id, tranche_id,
			)
		}

		/// Returns the tranche id at index 0 for the given pool id.
		pub fn default_tranche_id(pool_id: u64) -> TrancheId {
			let pool_details = PoolSystem::pool(pool_id).expect("Pool should exist");
			pool_details
				.tranches
				.tranche_id(TrancheLoc::Index(0))
				.expect("Tranche at index 0 exists")
		}

		/// Returns the derived general currency index.
		///
		/// Throws if the provided currency_id is not
		/// `CurrencyId::ForeignAsset(id)`.
		pub fn general_currency_index(currency_id: CurrencyId) -> u128 {
			pallet_liquidity_pools::Pallet::<DevelopmentRuntime>::try_get_general_index(currency_id)
				.expect("ForeignAsset should convert into u128")
		}

		/// Sets up required permissions for the investor and executes an
		/// initial investment via LiquidityPools by executing
		/// `IncreaseInvestOrder`.
		///
		/// Assumes `utils::setup_pre_requirements` and
		/// `utils::investments::create_currency_pool` to have been called
		/// beforehand
		pub fn do_initial_increase_investment(
			pool_id: u64,
			amount: Balance,
			investor: AccountId,
			currency_id: CurrencyId,
		) {
			let valid_until = utils::DEFAULT_VALIDITY;

			// Mock incoming increase invest message
			let msg = utils::LiquidityPoolMessage::IncreaseInvestOrder {
				pool_id,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(currency_id),
				amount,
			};
			// TODO(after PR #1376): Re-activate via Gateway handling
			// // Should fail if instance has not been added yet
			// assert_noop!(
			// 	LiquidityPools::submit(utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
			// 	pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidIncomingMessageOrigin
			// );
			// assert_ok!(LiquidityPools::add_instance(
			// 	RuntimeOrigin::root(),
			// 	utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM.address().into()
			// ));

			// Should fail if investor does not have investor role yet
			assert_noop!(
				LiquidityPools::submit(utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
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

			let amount_before = OrmlTokens::free_balance(
				currency_id,
				&investment_account(investment_id(pool_id, default_tranche_id(pool_id))),
			);
			let final_amount = amount_before
				.ensure_add(amount)
				.expect("Should not overflow when incrementing amount");

			// Execute byte message
			assert_ok!(LiquidityPools::submit(
				utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg
			));

			// Verify investment was transferred into investment account
			assert_eq!(
				OrmlTokens::free_balance(
					currency_id,
					&investment_account(investment_id(pool_id, default_tranche_id(pool_id)))
				),
				final_amount
			);
			assert_eq!(
				System::events().iter().last().unwrap().event,
				pallet_investments::Event::<DevelopmentRuntime>::InvestOrderUpdated {
					investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
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
		/// Assumes `utils::setup_pre_requirements` and
		/// `utils::investments::create_currency_pool` to have been called
		/// beforehand
		pub fn do_initial_increase_redemption(
			pool_id: u64,
			amount: Balance,
			investor: AccountId,
			currency_id: CurrencyId,
		) {
			let valid_until = utils::DEFAULT_VALIDITY;

			// Fund `DomainLocator` account of origination domain as redeemed tranche tokens
			// are transferred from this account instead of minting
			assert_ok!(OrmlTokens::mint_into(
				investment_id(pool_id, default_tranche_id(pool_id)).into(),
				&Domain::convert(DEFAULT_DOMAIN_ADDRESS_MOONBEAM.domain()),
				amount
			));

			// Verify redemption has not been made yet
			assert_eq!(
				OrmlTokens::free_balance(
					investment_id(pool_id, default_tranche_id(pool_id)).into(),
					&investment_account(investment_id(pool_id, default_tranche_id(pool_id)))
				),
				0
			);
			assert_eq!(
				OrmlTokens::free_balance(
					investment_id(pool_id, default_tranche_id(pool_id)).into(),
					&investor
				),
				0
			);

			// Mock incoming increase invest message
			let msg = utils::LiquidityPoolMessage::IncreaseRedeemOrder {
				pool_id: 42,
				tranche_id: default_tranche_id(pool_id),
				investor: investor.clone().into(),
				currency: general_currency_index(currency_id),
				amount,
			};
			// TODO(after PR #1376): Re-activate via Gateway handling
			// // Should fail if instance has not been added yet
			// assert_noop!(
			// 	LiquidityPools::submit(utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
			// 	pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidIncomingMessageOrigin
			// );
			// assert_ok!(LiquidityPools::add_instance(
			// 	RuntimeOrigin::root(),
			// 	utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM.address().into()
			// ));

			// Should fail if investor does not have investor role yet
			assert_noop!(
				LiquidityPools::submit(utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM, msg.clone()),
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

			assert_ok!(LiquidityPools::submit(
				utils::DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				msg
			));

			// Verify redemption was transferred into investment account
			assert_eq!(
				OrmlTokens::free_balance(
					investment_id(pool_id, default_tranche_id(pool_id)).into(),
					&investment_account(investment_id(pool_id, default_tranche_id(pool_id)))
				),
				amount
			);
			assert_eq!(
				OrmlTokens::free_balance(
					investment_id(pool_id, default_tranche_id(pool_id)).into(),
					&investor
				),
				0
			);
			assert_eq!(
				OrmlTokens::free_balance(
					investment_id(pool_id, default_tranche_id(pool_id)).into(),
					&AccountConverter::<DevelopmentRuntime>::convert(DEFAULT_OTHER_DOMAIN_ADDRESS)
				),
				0
			);
			assert_eq!(
				System::events().iter().last().unwrap().event,
				pallet_investments::Event::<DevelopmentRuntime>::RedeemOrderUpdated {
					investment_id: investment_id(pool_id, default_tranche_id(pool_id)),
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
}
