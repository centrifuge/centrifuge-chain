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


//! Crowdloan claim pallet's unit test cases


#![cfg(test)]


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{
    Error as CrowdloanClaimError,
    mock::*,
    self as pallet_crowdloan_claim,
    *
};

use frame_support::{
    assert_noop, 
    assert_ok,
};


// ----------------------------------------------------------------------------
// Test cases
// ----------------------------------------------------------------------------

// Test initializing the crowdloan claim pallet.
//
// After the crowdloan campaign is closed successfully, the list of contributors
// and their respective contributions (i.e. tokens locked on their Polkadot/Kusama
// relay chain account), must be stored on the parachain, so that to know who 
// contributed (and how much) and process claims for reward payouts.
// The [`initialize`] transaction can only be invoked once, to transfer the child-trie
// root hash containing the list of contributions from the relay chain's crowdloan
// pallet's child-trie storage item.
// sr_io::child_storage_root 
#[test]
fn test_valid_initialize_transaction() {
    TestExternalitiesBuilder::build().execute_with(|| {
        // TODO: Correct hash root here! Correct tri index!
		assert_ok!(CrowdloanClaim::initialize(Origin::signed(ADMIN_USER), 0, 0));
	})
}

#[test]
fn test_init_double() {
    // TODO: Init and then INit again
}

#[test]
fn test_invalid_signed_claim_transaction() {
    // TODO: Signed transaction, here which should fail
}

#[test]
fn test_valid_claim() {
    // TODO: Unsigend tx, with amount, proof etc, and previous init of module
}

#[test]
fn test_invalid_claim_invalid_proof() {
    // TODO: Init module, invalid proof for tx
}

#[test]
fn test_invalid_claim_wrong_amount() {
    // TODO: init module, valid proof, wrong amount
}

#[test]
fn test_invalid_claim_wrong_relayaccount() {
    // TODO: init module, valid proof, valid amount, wrong relay account
}

#[test]
fn test_invalid_claim_mod_not_initalized() {
    // TODO: claim wohtout init of module
}


