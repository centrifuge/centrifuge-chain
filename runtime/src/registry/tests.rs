use crate::registry::{Error, mock::*};
use crate::proofs;
use sp_core::{H256, U256, Encode};
use frame_support::{assert_err, assert_ok, Hashable};
use sp_runtime::{
    testing::Header,
    traits::{BadOrigin, BlakeTwo256, Hash, IdentityLookup, Block as BlockT},
};
use crate::nft;
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
fn proofs_data(registry_id: U256, token_id: Bytes) -> (Vec<Proof<H256>>, [H256; 3], H256) {
    let prop_vec = H256::from_low_u64_le(registry_id.as_u64()).as_bytes().into();
    let proofs = vec![
        Proof {
            value: token_id,
            salt: vec![0],
            property: prop_vec,//b"AMOUNT".to_vec(),
            hashes: vec![],
        }];
    let data_root    = proofs::Proof::from(proofs[0].clone()).leaf_hash;
    let zk_data_root = <Test as frame_system::Trait>::Hashing::hash_of(&0);
    let sig_root     = <Test as frame_system::Trait>::Hashing::hash_of(&0);
    //let zk_data_root = sp_io::hashing::keccak_256(&[0]).into();
    //let sig_root     = sp_io::hashing::keccak_256(&[0]).into();
    let static_hashes = [data_root, zk_data_root, sig_root];
    let doc_root     = doc_root(static_hashes);

    (proofs, static_hashes, doc_root)
}

#[test]
fn mint_with_valid_proofs_works() {
    new_test_ext().execute_with(|| {
        let owner     = 1;
        let origin    = Origin::signed(owner);
        let token_id  = vec![0];
        let registry_id = U256::zero();

        // Anchor data
        let pre_image = <Test as frame_system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as frame_system::Trait>::Hashing::hash);

        // Proofs data
        let (proofs, static_hashes, doc_root) = proofs_data(registry_id.clone(), token_id.clone());

        // Registry data
        let nft_data = AssetInfo {
            registry_id: registry_id,
            doc_root: doc_root.clone(),
            token_id: token_id,
        };
        let properties    =  proofs.iter().map(|p| p.property.clone()).collect();
        let registry_info = RegistryInfo {
            owner_can_burn: false,
            fields: properties,
        };

        // Starts with no Nfts
        assert_eq!(<nft::Module<Test>>::total(), 0);
        assert_eq!(<nft::Module<Test>>::total_for_account(owner), 0);

        // Create registry
        assert_ok!(
            SUT::create_registry(origin.clone(), registry_info)
        );

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
            <nft::Module<Test>>::account_for_asset::<H256>(<Test as frame_system::Trait>::Hashing::hash_of(&nft_data)),
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
        let owner     = 1;
        let origin    = Origin::signed(owner);
        let token_id  = vec![0];
        let registry_id = U256::zero();

        // Anchor data
        let pre_image = <Test as frame_system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as frame_system::Trait>::Hashing::hash);

        // Proofs data
        let (proofs, static_hashes, doc_root) = proofs_data(registry_id.clone(), token_id.clone());

        // Registry data
        let nft_data = AssetInfo {
            registry_id: registry_id,
            doc_root: doc_root.clone(),
            token_id: token_id,
        };
        let properties    =  proofs.iter().map(|p| p.property.clone()).collect();
        let registry_info = RegistryInfo {
            owner_can_burn: false,
            fields: properties,
        };

        // Create registry
        assert_ok!(SUT::create_registry(origin.clone(), registry_info));

        // Place document anchor into storage for verification
        assert_ok!( <anchor::Module<Test>>::commit(
            origin.clone(),
            pre_image.clone(),
            // Doc root
            <Test as frame_system::Trait>::Hashing::hash_of(&pre_image),
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
