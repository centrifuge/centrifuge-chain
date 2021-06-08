// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.


//! Bridge pallet's unit test cases


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{
    Error as BridgeError,
    mock::*,
    self as pallet_bridge,
    *
};

use frame_support::{
    assert_noop, 
    assert_ok,
};


// ----------------------------------------------------------------------------
// Test cases
// ----------------------------------------------------------------------------

#[test]
fn transfer_native() {
    new_test_ext().execute_with(|| {
        let dest_chain = 0;
        let resource_id = NativeTokenId::get();
        let amount: u128 = 20 * currency::RAD;
        let recipient = vec![99];

        assert_ok!(ChainBridge::whitelist_chain(Origin::root(), dest_chain.clone()));

        // Using account with not enough balance for fee should fail when requesting transfer
        assert_err!(
            Bridge::transfer_native(
                Origin::signed(RELAYER_C),
                amount.clone(),
                recipient.clone(),
                dest_chain,
            ),
            "Insufficient Balance"
        );

        let mut account_current_balance = <pallet_balances::Module<MockRuntime>>::free_balance(RELAYER_B);
        assert_eq!(account_current_balance, 100);

        // Using account with enough balance for fee but not for transfer amount
        assert_err!(
            Bridge::transfer_native(
                Origin::signed(RELAYER_B),
                amount.clone(),
                recipient.clone(),
                dest_chain,
            ),
            "Insufficient Balance"
        );

        // Account balance should be reverted to original balance
        account_current_balance = Balances::free_balance(RELAYER_B);
        assert_eq!(account_current_balance, 100);

        // Success
        assert_ok!(PalletBridge::transfer_native(
            Origin::signed(RELAYER_A),
            amount.clone(),
            recipient.clone(),
            dest_chain,
        ));

        expect_event(chainbridge::RawEvent::FungibleTransfer(
            dest_chain,
            1,
            resource_id,
            amount.into(),
            recipient,
        ));

        // Account balance should be reduced amount + fee
        account_current_balance = Balances::free_balance(RELAYER_A);
        assert_eq!(account_current_balance, 60 * currency::RAD);
    })
}

// Create a registry, set resource id and mint an nft.
fn setup_nft(owner: u64, token_id: U256, resource_id: ResourceId) -> RegistryId {
    let origin = Origin::signed(owner);

    // Create registry and generate proofs
    let (asset_id,
            pre_image,
            anchor_id,
            (proofs, static_hashes, doc_root),
            nft_data,
            _) = registry::tests::setup_mint::<MockRuntime>(owner, token_id);

    // Commit document root
    assert_ok!( <crate::anchor::Module<MockRuntime>>::commit(
        origin.clone(),
        pre_image,
        doc_root,
        <MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
        crate::common::MS_PER_DAY + 1));

    // Mint token with document proof
    let (registry_id, token_id) = asset_id.clone().destruct();
    assert_ok!(
        VaRegistry::mint(origin,
                    owner,
                    registry_id,
                    token_id,
                    nft_data.clone(),
                    registry::types::MintInfo {
                        anchor_id: anchor_id,
                        proofs: proofs,
                        static_hashes: static_hashes,
                    }));

    // Register resource with chainbridge
    assert_ok!(<chainbridge::Module<MockRuntime>>::register_resource(resource_id.clone(), vec![]));
    // Register resource in local resource mapping
    <bridge_mapping::Module<MockRuntime>>::set_resource(resource_id.clone(),
                                                registry_id.clone().into());

    registry_id
}

#[test]
fn receive_nonfungible() {
    new_test_ext().execute_with(|| {
        let dest_chain = 0;
        let resource_id = NativeTokenId::get();
        let recipient = RELAYER_A;
        let owner     = <chainbridge::Module<MockRuntime>>::account_id();
        let origin    = Origin::signed(owner);
        let token_id  = U256::one();

        // Create registry, map resource id, and mint nft
        let registry_id = setup_nft(owner, token_id, resource_id);

        // Whitelist destination chain
        assert_ok!(ChainBridge::whitelist_chain(Origin::root(), dest_chain.clone()));

        // Send nft from bridge account to user
        assert_ok!(<Module<MockRuntime>>::receive_nonfungible(origin,
                                                        recipient,
                                                        token_id,
                                                        vec![],
                                                        resource_id));

        // Recipient owns the nft now
        assert_eq!(<crate::nft::Module<MockRuntime>>::account_for_asset(registry_id, token_id),
                    Some(recipient));
    })
}

#[test]
fn transfer_nonfungible_asset() {
    new_test_ext().execute_with(|| {
        let dest_chain = 0;
        let resource_id = NativeTokenId::get();
        let recipient = vec![1];
        let owner = RELAYER_A;
        let token_id = U256::one();

        // Create registry, map resource id, and mint nft
        let registry_id = setup_nft(owner, token_id, resource_id);

        // Whitelist destination chain
        assert_ok!(ChainBridge::whitelist_chain(Origin::root(), dest_chain.clone()));

        // Owner owns nft
        assert_eq!(<crate::nft::Module<MockRuntime>>::account_for_asset(registry_id, token_id),
                    Some(owner));

        // Using account without enough balance for fee should fail when requesting transfer
        /*
        assert_err!(
            PalletBridge::transfer_asset(
                Origin::signed(RELAYER_C),
                recipient.clone(),
                registry_id,
                token_id.clone(),
                dest_chain),
            DispatchError::Module {
                index: 0,
                error: 3,
                Some("InsufficientBalance")});
        */

        // Transfer nonfungible through bridge
        assert_ok!(
            PalletBridge::transfer_asset(
                Origin::signed(owner),
                recipient.clone(),
                registry_id,
                token_id.clone(),
                dest_chain));

        // Now bridge module owns the nft
        assert_eq!(<crate::nft::Module<MockRuntime>>::account_for_asset(registry_id, token_id),
                    Some(<chainbridge::Module<MockRuntime>>::account_id()));

        // Check that transfer event was emitted
        let tid: &mut [u8] = &mut[0; 32];
        token_id.to_big_endian(tid);
        expect_event(chainbridge::RawEvent::NonFungibleTransfer(
            dest_chain,
            1,
            resource_id,
            tid.to_vec(),
            recipient,
            vec![],
        ));
    })
}


#[test]
fn execute_remark() {
    new_test_ext().execute_with(|| {
        let hash: H256 = "ABC".using_encoded(blake2_256).into();
        let prop_id = 1;
        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"hash");
        let proposal = make_remark_proposal(hash.clone(), r_id);
        let resource = b"PalletBridge.remark".to_vec();

        assert_ok!(ChainBridge::set_threshold(Origin::root(), TEST_THRESHOLD,));
        assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_A));
        assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_B));
        assert_ok!(ChainBridge::whitelist_chain(Origin::root(), src_id));
        assert_ok!(ChainBridge::set_resource(Origin::root(), r_id, resource));

        assert_ok!(ChainBridge::acknowledge_proposal(
            Origin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        assert_ok!(ChainBridge::acknowledge_proposal(
            Origin::signed(RELAYER_B),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));

        event_exists(RawEvent::Remark(hash, r_id));
    })
}

#[test]
fn execute_remark_bad_origin() {
    new_test_ext().execute_with(|| {
        let hash: H256 = "ABC".using_encoded(blake2_256).into();
        let r_id = chainbridge::derive_resource_id(1, b"hash");

        assert_ok!(PalletBridge::remark(Origin::signed(ChainBridge::account_id()), hash, r_id));
        // Don't allow any signed origin except from chainbridge addr
        assert_noop!(
            PalletBridge::remark(Origin::signed(RELAYER_A), hash, r_id),
            DispatchError::BadOrigin
        );
        // Don't allow root calls
        assert_noop!(
            PalletBridge::remark(Origin::root(), hash, r_id),
            DispatchError::BadOrigin
        );
    })
}

#[test]
fn transfer() {
    new_test_ext().execute_with(|| {
        // Check inital state
        let bridge_id: u64 = ChainBridge::account_id();
        let resource_id = NativeTokenId::get();
        assert_eq!(Balances::free_balance(&bridge_id), ENDOWED_BALANCE);
        // Transfer and check result
        assert_ok!(PalletBridge::transfer(
            Origin::signed(ChainBridge::account_id()),
            RELAYER_A,
            10,
            resource_id
        ));
        assert_eq!(Balances::free_balance(&bridge_id), ENDOWED_BALANCE - 10);
        assert_eq!(Balances::free_balance(RELAYER_A), ENDOWED_BALANCE + 10);

        assert_events(vec![Event::balances(balances::RawEvent::Transfer(
            ChainBridge::account_id(),
            RELAYER_A,
            10,
        ))]);
    })
}

#[test]
fn create_successful_transfer_proposal() {
    new_test_ext().execute_with(|| {
        let prop_id = 1;
        let src_id = 1;
        let r_id = chainbridge::derive_resource_id(src_id, b"transfer");
        let resource = b"PalletBridge.transfer".to_vec();
        let proposal = make_transfer_proposal(RELAYER_A, 10, r_id);

        assert_ok!(ChainBridge::set_threshold(Origin::root(), TEST_THRESHOLD,));
        assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_A));
        assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_B));
        assert_ok!(ChainBridge::add_relayer(Origin::root(), RELAYER_C));
        assert_ok!(ChainBridge::whitelist_chain(Origin::root(), src_id));
        assert_ok!(ChainBridge::set_resource(Origin::root(), r_id, resource));

        // Create proposal (& vote)
        assert_ok!(ChainBridge::acknowledge_proposal(
            Origin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = chainbridge::ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: chainbridge::ProposalStatus::Initiated,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        // Second relayer votes against
        assert_ok!(ChainBridge::reject_proposal(
            Origin::signed(RELAYER_B),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = chainbridge::ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![RELAYER_B],
            status: chainbridge::ProposalStatus::Initiated,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        // Third relayer votes in favour
        assert_ok!(ChainBridge::acknowledge_proposal(
            Origin::signed(RELAYER_C),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = ChainBridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = chainbridge::ProposalVotes {
            votes_for: vec![RELAYER_A, RELAYER_C],
            votes_against: vec![RELAYER_B],
            status: chainbridge::ProposalStatus::Approved,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        assert_eq!(Balances::free_balance(RELAYER_A), ENDOWED_BALANCE + 10);
        assert_eq!(
            Balances::free_balance(ChainBridge::account_id()),
            ENDOWED_BALANCE - 10
        );

        assert_events(vec![
            Event::chainbridge(chainbridge::RawEvent::VoteFor(src_id, prop_id, RELAYER_A)),
            Event::chainbridge(chainbridge::RawEvent::VoteAgainst(src_id, prop_id, RELAYER_B)),
            Event::chainbridge(chainbridge::RawEvent::VoteFor(src_id, prop_id, RELAYER_C)),
            Event::chainbridge(chainbridge::RawEvent::ProposalApproved(src_id, prop_id)),
            Event::balances(balances::RawEvent::Transfer(
                ChainBridge::account_id(),
                RELAYER_A,
                10,
            )),
            Event::chainbridge(chainbridge::RawEvent::ProposalSucceeded(src_id, prop_id)),
        ]);
    })
}