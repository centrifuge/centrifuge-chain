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


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use frame_support::{
  assert_ok, 
  assert_err 
};


// ----------------------------------------------------------------------------
// Mock runtime environment
// ----------------------------------------------------------------------------

// Mock runtime storage
pub struct ExtBuilder;

impl ExtBuilder {

  // Build 
  pub fn build() -> sp_io::TestExternalities {
    
    // create a fake storage root as one created in crowdloan module for storing contributions 
    let mut storage = system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();
    
    sp_io::TestExternalities::from(storage)
  }
}


// ----------------------------------------------------------------------------
// Unit tests and fixtures
// ----------------------------------------------------------------------------

// Test crowdloan storage root upload to the parachain.
//
// After the crowdloan campaign is closed successfully, the list of contributors
// and their respective contributions (i.e. tokens locked on their Polkadot/Kusama
// relay chain account), must be stored in the parachain, so that to be able to 
// reward contributor for forgoing staking.
// sr_io::child_storage_root 
#[test]
fn store_storage_root() {
    new_test_ext().execute_with(|| {
        let admin       = Origin::root();
        assert_ok!( SUT::set(admin.clone(), 1, 2) );
        assert_ok!( SUT::set(admin        , 1, 3) );

        // Check that resource mapping was added to storage
        assert_eq!(SUT::addr_of(1), Some(3));
        assert_eq!(SUT::name_of(3), Some(1));
    });
}
