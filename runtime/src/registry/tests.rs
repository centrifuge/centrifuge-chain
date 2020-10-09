use crate::registry::{Error, mock::*, types::AssetId};
use crate::nft::Error as NftError;
use crate::proofs;
use sp_core::{H256, U256, Encode};
use frame_support::{assert_err, assert_ok, Hashable};
use sp_runtime::{
    testing::Header,
    traits::{BadOrigin, BlakeTwo256, Hash, IdentityLookup, Block as BlockT},
};
use crate::nft::{self, DefaultInstance};
use super::*;

// Hash two hashes
fn hash_of(a: H256, b: H256) -> H256 {
    let mut h: Vec<u8> = Vec::with_capacity(64);
    h.extend_from_slice(&a[..]);
    h.extend_from_slice(&b[..]);
    sp_io::hashing::blake2_256(&h).into()
}
// Generate document root from static hashes
fn doc_root(static_hashes: [H256; 3]) -> H256 {
    let basic_data_root = static_hashes[0];
    let zk_data_root    = static_hashes[1];
    let signature_root  = static_hashes[2];
    let signing_root    = hash_of(basic_data_root, zk_data_root);
    hash_of(signing_root, signature_root)
}

// Some dummy proofs data useful for testing. Returns proofs, static hashes, and document root
fn proofs_data(registry_id: H160, token_id: AssetId) -> (Vec<Proof<H256>>, [H256; 3], H256) {
    let token_id = H256::from_low_u64_le(token_id.as_u64()).as_bytes().into();
    let proofs = vec![
        Proof {
            value: token_id,
            salt: vec![0],
            property: registry_id.as_bytes().into(),//b"AMOUNT".to_vec(),
            hashes: vec![],
        }];
    let data_root    = proofs::Proof::from(proofs[0].clone()).leaf_hash;
    let zk_data_root = <Test as frame_system::Trait>::Hashing::hash_of(&0);
    let sig_root     = <Test as frame_system::Trait>::Hashing::hash_of(&0);
    let static_hashes = [data_root, zk_data_root, sig_root];
    let doc_root     = doc_root(static_hashes);

    (proofs, static_hashes, doc_root)
}

// Creates a registry and returns all relevant data
fn setup_mint() -> (u64, Origin, U256,
                    H160, H256, H256,
                    (Vec<Proof<H256>>,
                     [H256; 3], H256),
                    crate::registry::types::AssetInfo,
                    crate::registry::types::RegistryInfo) {
    let owner     = 1;
    let origin    = Origin::signed(owner);
    let asset_id  = U256::zero();
    let metadata  = vec![];
    let registry_id = H160::zero();

    // Anchor data
    let pre_image = <Test as frame_system::Trait>::Hashing::hash_of(&0);
    let anchor_id = (pre_image).using_encoded(<Test as frame_system::Trait>::Hashing::hash);

    // Proofs data
    let (proofs, static_hashes, doc_root) = proofs_data(registry_id.clone(), asset_id.clone());

    // Registry data
    let nft_data = AssetInfo {
        registry_id,
        asset_id,
        metadata,
    };
    let properties    =  proofs.iter().skip(1).map(|p| p.property.clone()).collect();
    let registry_info = RegistryInfo {
        owner_can_burn: false,
        // Don't include the registry id prop which will be generated in the runtime
        fields: properties,
    };

    // Create registry
    assert_ok!(
        SUT::create_registry(origin.clone(), registry_info.clone())
    );

    (owner,
     origin,
     asset_id,
     registry_id,
     pre_image,
     anchor_id,
     (proofs, static_hashes, doc_root),
     nft_data,
     registry_info)
}

#[test]
fn mint_with_valid_proofs_works() {
    new_test_ext().execute_with(|| {
        let (owner,
             origin,
             asset_id,
             registry_id,
             pre_image,
             anchor_id,
             (proofs, static_hashes, doc_root),
             nft_data,
             registry_info) = setup_mint();

        // Starts with no Nfts
        assert_eq!(<nft::Module<Test>>::total(), 0);
        assert_eq!(<nft::Module<Test>>::total_for_account(owner), 0);

        // Place document anchor into storage for verification
        assert_ok!( <anchor::Module<Test>>::commit(
            origin.clone(),
            pre_image,
            doc_root,
            // Proof does not matter here
            <Test as frame_system::Trait>::Hashing::hash_of(&0),
            crate::common::MS_PER_DAY + 1) );

        // Mint token with document proof
        assert_ok!(
            SUT::mint(origin,
                      owner,
                      nft_data.clone(),
                      MintInfo {
                          anchor_id: anchor_id,
                          proofs: proofs,
                          static_hashes: static_hashes,
                      }));

        // Nft registered to owner
        assert_eq!(
            <nft::Module<Test>>::account_for_asset::<H160,U256>(registry_id, asset_id),
            owner
        );

        // Total Nfts did increase
        assert_eq!(<nft::Module<Test>>::total(), 1);
        assert_eq!(<nft::Module<Test>>::total_for_account(owner), 1);
    });
}

#[test]
fn mint_fails_when_dont_match_doc_root() {
    new_test_ext().execute_with(|| {
        let (owner,
             origin,
             asset_id,
             registry_id,
             pre_image,
             anchor_id,
             (proofs, static_hashes, doc_root),
             nft_data,
             registry_info) = setup_mint();

        // Place document anchor into storage for verification
        let wrong_doc_root = <Test as frame_system::Trait>::Hashing::hash_of(&pre_image);
        assert_ok!( <anchor::Module<Test>>::commit(
            origin.clone(),
            pre_image.clone(),
            wrong_doc_root,
            // Proof does not matter here
            <Test as frame_system::Trait>::Hashing::hash_of(&0),
            crate::common::MS_PER_DAY + 1) );

        // Mint token with document proof
        assert_err!(
            SUT::mint(origin,
                      owner,
                      nft_data,
                      MintInfo {
                          anchor_id: anchor_id,
                          proofs: proofs,
                          static_hashes: static_hashes,
                      }),
            Error::<Test>::InvalidProofs);
    });
}

#[test]
fn duplicate_mint_fails() {
    new_test_ext().execute_with(|| {
        let (owner,
             origin,
             asset_id,
             registry_id,
             pre_image,
             anchor_id,
             (proofs, static_hashes, doc_root),
             nft_data,
             registry_info) = setup_mint();

        // Place document anchor into storage for verification
        assert_ok!( <anchor::Module<Test>>::commit(
            origin.clone(),
            pre_image,
            doc_root,
            // Proof does not matter here
            <Test as frame_system::Trait>::Hashing::hash_of(&0),
            crate::common::MS_PER_DAY + 1) );

        // Mint token with document proof
        assert_ok!(
            SUT::mint(origin.clone(),
                      owner,
                      nft_data.clone(),
                      MintInfo {
                          anchor_id: anchor_id,
                          proofs: proofs.clone(),
                          static_hashes: static_hashes,
                      }));

        // Mint same token containing same id
        assert_err!(
            SUT::mint(origin,
                      owner,
                      nft_data.clone(),
                      MintInfo {
                          anchor_id: anchor_id,
                          proofs: proofs,
                          static_hashes: static_hashes,
                      }),
            NftError::<Test, DefaultInstance>::AssetExists);
    });
}

/*
#[test]
fn burn_nft_works() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1);

        let fields = vec![b"AMOUNT".into()];
        let registry_info = RegistryInfo {
            owner_can_burn: false,
            fields: fields,
        };

        // Create registry
        assert_ok!(
            SUT::create_registry(origin.clone(), registry_info)
        );

        assert_ok!(
            SUT::burn(
    });
}
*/
