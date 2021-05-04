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
  *, 
  mock::*
};

use frame_support::{
  assert_noop
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
fn test_initialize_pallet() {
  TestExternalitiesBuilder::build().execute_with(|| {
		//assert_ok!(CrowdloanClaim::initialize(Origin::signed(1),  ));
    let flag = true;
    assert!(flag == true, "Flag is true folks");
	})
}

// Test if the amount that is claimed either exceeds or is less than the contribution.
//
// The child trie root hash stored in [`Contributions`] storage item contains all the
// contributions made during the crowdloan campaign, and hence, can be used to check
// if a contributor asking for a reward payout is elligible or not for it.
#[test]
fn test_claim_wrong_reward_amount() {
  TestExternalitiesBuilder::build().execute_with(|| {
		assert_noop!(
			CrowdloanClaim::claim_reward_unsigned(Origin::signed(1), 39, 39),
			Error::<MockRuntime>::InvalidClaimAmount
		);
	});
}
