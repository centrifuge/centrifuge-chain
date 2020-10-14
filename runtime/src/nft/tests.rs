use super::*;
use crate::nft::mock::*;
use unique_assets::traits::*;
use sp_core::{H160, U256};
use crate::registry::types::AssetId;
use frame_support::{assert_err, assert_ok, Hashable};

#[test]
fn mint() {
    new_test_ext().execute_with(|| {
        let from = 0;
        let to = 1;
        let asset_id = AssetId(H160::zero(), U256::zero());
        let asset_info = vec![];
        assert_ok!(<SUT as Mintable>::mint(&0, &1, &asset_id, asset_info));
    });
}

/*
#[test]
fn transfer() {
    new_test_ext().execute_with(|| {
    });
}
*/
