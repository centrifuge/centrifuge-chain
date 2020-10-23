use super::*;
use crate::nft::mock::*;
use unique_assets::traits::*;
use sp_core::{H160, U256};
use crate::va_registry::types::AssetId;
use frame_support::{assert_err, assert_ok, Hashable};

#[test]
fn mint() {
    new_test_ext().execute_with(|| {
        let asset_id = AssetId(H160::zero(), U256::zero());
        let asset_info = vec![];
        assert_ok!(<SUT as Mintable>::mint(&0, &1, &asset_id, asset_info));
    });
}

#[test]
fn mint_err_duplicate_id() {
    new_test_ext().execute_with(|| {
        let asset_id = AssetId(H160::zero(), U256::zero());
        assert_ok!(<SUT as Mintable>::mint(&0, &1, &asset_id, vec![]));
        assert_err!(<SUT as Mintable>::mint(&0, &1, &asset_id, vec![]),
                    Error::<Test>::AssetExists);
    });
}

#[test]
fn transfer() {
    new_test_ext().execute_with(|| {
        let asset_id = AssetId(H160::zero(), U256::zero());
        // First mint to account 1
        assert_ok!(<SUT as Mintable>::mint(&1, &1, &asset_id, vec![]));
        // Transfer to 2
        assert_ok!(<SUT as Unique>::transfer(&1, &2, &asset_id));
        // 2 owns asset now
        assert_eq!(<SUT as Unique>::owner_of(&asset_id), 2);
    });
}

#[test]
fn transfer_err_on_default_acct() {
    new_test_ext().execute_with(|| {
        let asset_id = AssetId(H160::zero(), U256::zero());
        // Mint to account 0, default account
        assert_ok!(<SUT as Mintable>::mint(&0, &0, &asset_id, vec![]));
        // 0 transfers to 1
        assert_err!(<SUT as Unique>::transfer(&0, &1, &asset_id),
                    Error::<Test>::NonexistentAsset);
    });
}

#[test]
fn transfer_err_when_not_owner() {
    new_test_ext().execute_with(|| {
        let asset_id = AssetId(H160::zero(), U256::zero());
        // Mint to account 2
        assert_ok!(<SUT as Mintable>::mint(&2, &2, &asset_id, vec![]));
        // 1 transfers to 2
        assert_err!(<SUT as Unique>::transfer(&1, &2, &asset_id),
                    Error::<Test>::NotAssetOwner);
    });
}
