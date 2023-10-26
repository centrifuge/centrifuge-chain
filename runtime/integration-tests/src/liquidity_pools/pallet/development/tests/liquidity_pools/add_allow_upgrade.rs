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
use development_runtime::{
	LiquidityPools, LocationToAccountId, OrderBook, OrmlAssetRegistry, OrmlTokens, Permissions,
	Runtime as DevelopmentRuntime, RuntimeOrigin, System, TreasuryAccount, XTokens, XcmTransactor,
};
use frame_support::{assert_noop, assert_ok, traits::fungibles::Mutate};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::account_conversion::AccountConverter;
use sp_runtime::{
	traits::{BadOrigin, Convert, One, Zero},
	BoundedVec, DispatchError,
};
use xcm::{latest::MultiLocation, VersionedMultiLocation};
use xcm_emulator::TestExt;

use crate::{
	liquidity_pools::pallet::development::{
		setup::{dollar, ALICE, BOB},
		test_net::{Development, Moonbeam, RelayChain, TestNet},
		tests::liquidity_pools::setup::{
			asset_metadata, create_ausd_pool, create_currency_pool,
			enable_liquidity_pool_transferability, get_default_moonbeam_native_token_location,
			investments::default_tranche_id, liquidity_pools_transferable_multilocation,
			setup_pre_requirements, DEFAULT_BALANCE_GLMR, DEFAULT_MOONBEAM_LOCATION,
			DEFAULT_POOL_ID, DEFAULT_VALIDITY,
		},
	},
	utils::{AUSD_CURRENCY_ID, GLMR_CURRENCY_ID, MOONBEAM_EVM_CHAIN_ID},
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
		setup_pre_requirements();
		let pool_id = DEFAULT_POOL_ID;

		// Verify that the pool must exist before we can call LiquidityPools::add_pool
		assert_noop!(
			LiquidityPools::add_pool(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::PoolNotFound
		);

		// Now create the pool
		create_ausd_pool(pool_id);

		// Verify ALICE can't call `add_pool` given she is not the `PoolAdmin`
		assert_noop!(
			LiquidityPools::add_pool(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::NotPoolAdmin
		);

		// Verify that it works if it's BOB calling it (the pool admin)
		assert_ok!(LiquidityPools::add_pool(
			RuntimeOrigin::signed(BOB.into()),
			pool_id,
			Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
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
		setup_pre_requirements();
		let decimals: u8 = 15;

		// Now create the pool
		let pool_id = DEFAULT_POOL_ID;
		create_ausd_pool(pool_id);

		// Verify we can't call LiquidityPools::add_tranche with a non-existing
		// tranche_id
		let nonexistent_tranche = [71u8; 16];
		assert_noop!(
			LiquidityPools::add_tranche(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				nonexistent_tranche,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::TrancheNotFound
		);
		let tranche_id = default_tranche_id(pool_id);

		// Verify ALICE can't call `add_tranche` given she is not the `PoolAdmin`
		assert_noop!(
			LiquidityPools::add_tranche(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				tranche_id,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::NotPoolAdmin
		);

		// Finally, verify we can call LiquidityPools::add_tranche successfully
		// when called by the PoolAdmin with the right pool + tranche id pair.
		assert_ok!(LiquidityPools::add_tranche(
			RuntimeOrigin::signed(BOB.into()),
			pool_id,
			tranche_id,
			Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
		));

		// Edge case: Should throw if tranche exists but metadata does not exist
		let tranche_currency_id = CurrencyId::Tranche(pool_id, tranche_id);
		orml_asset_registry::Metadata::<DevelopmentRuntime>::remove(tranche_currency_id);
		assert_noop!(
			LiquidityPools::update_tranche_token_metadata(
				RuntimeOrigin::signed(BOB.into()),
				pool_id,
				tranche_id,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::TrancheMetadataNotFound
		);
	});
}

#[test]
fn update_member() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();

		// Now create the pool
		let pool_id = DEFAULT_POOL_ID;
		create_ausd_pool(pool_id);
		let tranche_id = default_tranche_id(pool_id);

		// Finally, verify we can call LiquidityPools::add_tranche successfully
		// when given a valid pool + tranche id pair.
		let new_member = DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [3; 20]);
		let valid_until = DEFAULT_VALIDITY;

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
			AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert(
				new_member.clone()
			),
			PermissionScope::Pool(pool_id),
			Role::PoolRole(PoolRole::TrancheInvestor(
				default_tranche_id(pool_id),
				valid_until
			)),
		));

		// Verify the Investor role was set as expected in Permissions
		assert!(Permissions::has(
			PermissionScope::Pool(pool_id),
			AccountConverter::<DevelopmentRuntime, LocationToAccountId>::convert(
				new_member.clone()
			),
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
				DomainAddress::EVM(MOONBEAM_EVM_CHAIN_ID, [9; 20]),
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
		setup_pre_requirements();
		let decimals: u8 = 15;
		let currency_id = AUSD_CURRENCY_ID;
		let pool_id = DEFAULT_POOL_ID;
		create_ausd_pool(pool_id);
		enable_liquidity_pool_transferability(currency_id);

		assert_ok!(LiquidityPools::update_token_price(
			RuntimeOrigin::signed(BOB.into()),
			pool_id,
			default_tranche_id(pool_id),
			currency_id,
			Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
		));
	});
}

#[test]
fn add_currency() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();

		let currency_id = AUSD_CURRENCY_ID;

		// Enable LiquidityPools transferability
		enable_liquidity_pool_transferability(currency_id);

		assert_eq!(
			OrmlTokens::free_balance(
				GLMR_CURRENCY_ID,
				&<DevelopmentRuntime as pallet_liquidity_pools_gateway::Config>::Sender::get()
			),
			DEFAULT_BALANCE_GLMR
		);

		assert_ok!(LiquidityPools::add_currency(
			RuntimeOrigin::signed(BOB.into()),
			currency_id
		));

		assert_eq!(
			OrmlTokens::free_balance(
				GLMR_CURRENCY_ID,
				&<DevelopmentRuntime as pallet_liquidity_pools_gateway::Config>::Sender::get()
			),
			/// Ensure it only charged the 0.2 GLMR of fee
			DEFAULT_BALANCE_GLMR
				- dollar(18).saturating_div(5)
		);
	});
}

#[test]
fn add_currency_should_fail() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();

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
			asset_metadata(
				"Test".into(),
				"TEST".into(),
				12,
				false,
				1_000_000,
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
			Some(Some(liquidity_pools_transferable_multilocation(
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
	});
}

#[test]
fn allow_investment_currency() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();

		let currency_id = AUSD_CURRENCY_ID;
		let pool_id = DEFAULT_POOL_ID;
		let evm_chain_id: u64 = MOONBEAM_EVM_CHAIN_ID;
		let evm_address = [1u8; 20];

		// Create an AUSD pool
		create_ausd_pool(pool_id);

		// Enable LiquidityPools transferability
		assert_ok!(OrmlAssetRegistry::update_asset(
			RuntimeOrigin::root(),
			currency_id,
			None,
			None,
			None,
			None,
			// Changed: Add location which can be converted to LiquidityPoolsWrappedToken
			Some(Some(liquidity_pools_transferable_multilocation(
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

		assert_ok!(LiquidityPools::allow_investment_currency(
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
		let pool_id = DEFAULT_POOL_ID;
		let currency_id = CurrencyId::ForeignAsset(42);
		let ausd_currency_id = AUSD_CURRENCY_ID;

		setup_pre_requirements();
		// Should fail if pool does not exist
		assert_noop!(
			LiquidityPools::allow_investment_currency(
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
			asset_metadata(
				"Test".into(),
				"TEST".into(),
				12,
				true,
				1_000_000,
				None,
				Default::default(),
			),
			Some(currency_id)
		));

		// Create pool
		create_currency_pool(pool_id, currency_id, 10_000 * dollar(12));

		// Should fail if asset is not payment currency
		assert!(currency_id != ausd_currency_id);
		assert_noop!(
			LiquidityPools::allow_investment_currency(
				RuntimeOrigin::signed(BOB.into()),
				pool_id,
				default_tranche_id(pool_id),
				ausd_currency_id,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidPaymentCurrency
		);

		// Allow as payment but not payout currency
		assert_ok!(OrderBook::add_trading_pair(
			RuntimeOrigin::root(),
			currency_id,
			ausd_currency_id,
			Default::default()
		));
		// Should fail if asset is not payout currency
		enable_liquidity_pool_transferability(ausd_currency_id);
		assert_noop!(
			LiquidityPools::allow_investment_currency(
				RuntimeOrigin::signed(BOB.into()),
				pool_id,
				default_tranche_id(pool_id),
				ausd_currency_id,
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::InvalidPayoutCurrency
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
			LiquidityPools::allow_investment_currency(
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
			LiquidityPools::allow_investment_currency(
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
			LiquidityPools::allow_investment_currency(
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
			asset_metadata(
				"Acala Dollar".into(),
				"AUSD".into(),
				12,
				true,
				1_000_000,
				None,
				Default::default()
			),
			Some(CurrencyId::AUSD)
		));
		create_currency_pool(pool_id + 1, CurrencyId::AUSD, 10_000 * dollar(12));
		// Should fail if currency is not foreign asset
		assert_noop!(
			LiquidityPools::allow_investment_currency(
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
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();

		// Only Root can call `schedule_upgrade`
		assert_noop!(
			LiquidityPools::schedule_upgrade(
				RuntimeOrigin::signed(BOB.into()),
				MOONBEAM_EVM_CHAIN_ID,
				[7; 20]
			),
			BadOrigin
		);

		// Now it finally works
		assert_ok!(LiquidityPools::schedule_upgrade(
			RuntimeOrigin::root(),
			MOONBEAM_EVM_CHAIN_ID,
			[7; 20]
		));
	});
}

#[test]
fn cancel_upgrade_upgrade() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();

		// Only Root can call `cancel_upgrade`
		assert_noop!(
			LiquidityPools::cancel_upgrade(
				RuntimeOrigin::signed(BOB.into()),
				MOONBEAM_EVM_CHAIN_ID,
				[7; 20]
			),
			BadOrigin
		);

		// Now it finally works
		assert_ok!(LiquidityPools::cancel_upgrade(
			RuntimeOrigin::root(),
			MOONBEAM_EVM_CHAIN_ID,
			[7; 20]
		));
	});
}

#[test]
fn update_tranche_token_metadata() {
	TestNet::reset();
	Development::execute_with(|| {
		setup_pre_requirements();
		let decimals: u8 = 15;
		let pool_id = DEFAULT_POOL_ID;
		// NOTE: Default pool admin is BOB
		create_ausd_pool(pool_id);

		// Missing tranche token should throw
		let nonexistent_tranche = [71u8; 16];
		assert_noop!(
			LiquidityPools::update_tranche_token_metadata(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				nonexistent_tranche,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::TrancheNotFound
		);
		let tranche_id = default_tranche_id(pool_id);

		// Should throw if called by anything but `PoolAdmin`
		assert_noop!(
			LiquidityPools::update_tranche_token_metadata(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				tranche_id,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::NotPoolAdmin
		);

		assert_ok!(LiquidityPools::update_tranche_token_metadata(
			RuntimeOrigin::signed(BOB.into()),
			pool_id,
			tranche_id,
			Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
		));

		// Edge case: Should throw if tranche exists but metadata does not exist
		let tranche_currency_id = CurrencyId::Tranche(pool_id, tranche_id);
		orml_asset_registry::Metadata::<DevelopmentRuntime>::remove(tranche_currency_id);
		assert_noop!(
			LiquidityPools::update_tranche_token_metadata(
				RuntimeOrigin::signed(BOB.into()),
				pool_id,
				tranche_id,
				Domain::EVM(MOONBEAM_EVM_CHAIN_ID),
			),
			pallet_liquidity_pools::Error::<DevelopmentRuntime>::TrancheMetadataNotFound
		);
	});
}
