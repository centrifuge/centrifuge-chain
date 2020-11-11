use crate::proofs;
use sp_runtime::traits::Hash;
use sp_core::{H256, U256, Encode};
use crate::nft::Error as NftError;
use frame_support::{assert_err, assert_ok};
use crate::va_registry::{
    self, Error, mock::*,
    types::{AssetId, NFTS_PREFIX},
};
use crate::nft;
use super::*;

// Hash two hashes
fn hash_of<T: frame_system::Trait>(a: H256, b: H256) -> T::Hash {
    let mut h: Vec<u8> = Vec::with_capacity(64);
    h.extend_from_slice(&a[..]);
    h.extend_from_slice(&b[..]);
    T::Hashing::hash(&h)
}
// Generate document root from static hashes
fn doc_root<T: frame_system::Trait>(static_hashes: [H256; 3]) -> T::Hash {
    let basic_data_root = static_hashes[0];
    let zk_data_root    = static_hashes[1];
    let signature_root  = static_hashes[2];
    let signing_root    = H256::from_slice( hash_of::<T>(basic_data_root, zk_data_root).as_ref() );
    hash_of::<T>(signing_root, signature_root)
}

// Some dummy proofs data useful for testing. Returns proofs, static hashes, and document root
fn proofs_data<T: frame_system::Trait>(registry_id: RegistryId, token_id: TokenId)
    -> (Vec<Proof<H256>>, [H256; 3], T::Hash) {
    // Encode token into big endian U256
    let mut token_enc = Vec::<u8>::with_capacity(32);
    unsafe { token_enc.set_len(32); }
    token_id.to_big_endian(&mut token_enc);

    // Pre proof has registry_id: token_id as prop: value
    let pre_proof = Proof {
        value: token_enc,
        salt: [1; 32],
        property: [NFTS_PREFIX, registry_id.as_bytes()].concat(),
        hashes: vec![]};

    let proofs = vec![
        Proof {
            value: vec![1,1],
            salt: [1; 32],
            property: b"AMOUNT".to_vec(),
            hashes: vec![proofs::Proof::from(pre_proof.clone()).leaf_hash],
        },
        pre_proof.clone()
    ];
    let mut leaves: Vec<H256> = proofs.iter().map(|p| proofs::Proof::from(p.clone()).leaf_hash).collect();
    leaves.sort();

    let mut h: Vec<u8> = Vec::with_capacity(64);
    h.extend_from_slice(&leaves[0][..]);
    h.extend_from_slice(&leaves[1][..]);
    let data_root     = sp_io::hashing::blake2_256(&h).into();
    let zk_data_root  = sp_io::hashing::blake2_256(&[0]).into();
    let sig_root      = sp_io::hashing::blake2_256(&[0]).into();
    let static_hashes = [data_root, zk_data_root, sig_root];
    let doc_root      = doc_root::<T>(static_hashes);

    (proofs, static_hashes, doc_root)
}

// Creates a registry and returns all relevant data
pub fn setup_mint<T>(origin: T::Origin, token_id: TokenId)
    -> (AssetId,
        T::Hash, T::Hash,
        (Vec<Proof<H256>>, [H256; 3], T::Hash),
        va_registry::types::AssetInfo,
        va_registry::types::RegistryInfo)
    where T: frame_system::Trait
           + va_registry::Trait
           + nft::Trait<AssetInfo = types::AssetInfo>
{
    let metadata  = vec![];

    // Anchor data
    let pre_image = T::Hashing::hash(&[0]);
    let anchor_id = (pre_image).using_encoded(T::Hashing::hash);

    // Registry info
    let properties = vec![b"AMOUNT".to_vec()];
    let registry_info = RegistryInfo {
        owner_can_burn: false,
        // Don't include the registry id prop which will be generated in the runtime
        fields: properties,
    };

    // Create registry, get registry id
    assert_ok!( <va_registry::Module<T>>::create_registry(origin, registry_info.clone()) );

    // Same as registry id but an H160 instead of an associated type
    // Assumes the created registry is the first ever
    let mut res = Vec::<u8>::with_capacity(32);
    unsafe { res.set_len(32); }
    U256::from_big_endian(H160::zero().as_bytes())
         .saturating_add(U256::one())
         .to_big_endian(&mut res);
    let registry_id = H160::from_slice(&res[0..20]);

    // Proofs data
    let (proofs, static_hashes, doc_root) = proofs_data::<T>(registry_id.clone(), token_id.clone());

    // Registry data
    let nft_data = AssetInfo {
        metadata,
    };

    // Asset id
    let asset_id = AssetId(registry_id, token_id);

    (asset_id,
     pre_image,
     anchor_id,
     (proofs, static_hashes, doc_root),
     nft_data,
     registry_info)
}

#[test]
fn mint_with_valid_proofs_works() {
    new_test_ext().execute_with(|| {
        let token_id = U256::one();
        let owner = 1;
        let origin = Origin::signed(owner);
        let (asset_id,
             pre_image,
             anchor_id,
             (proofs, static_hashes, doc_root),
             nft_data,
             _) = setup_mint::<Test>(origin.clone(), token_id);

        // Place document anchor into storage for verification
        assert_ok!( <anchor::Module<Test>>::commit(
            origin.clone(),
            pre_image,
            doc_root,
            // Proof does not matter here
            <Test as frame_system::Trait>::Hashing::hash_of(&0),
            crate::common::MS_PER_DAY + 1) );

        let (registry_id, token_id) = asset_id.destruct();

        // Mint token with document proof
        assert_ok!(
            SUT::mint(origin,
                      owner,
                      registry_id,
                      token_id,
                      nft_data.clone(),
                      MintInfo {
                          anchor_id: anchor_id,
                          proofs: proofs,
                          static_hashes: static_hashes,
                      }));

        // Nft registered to owner
        assert_eq!(
            <nft::Module<Test>>::account_for_asset::<H160,U256>(registry_id, token_id),
            owner
        );
    });
}

#[test]
fn mint_fails_when_dont_match_doc_root() {
    new_test_ext().execute_with(|| {
        let token_id = U256::one();
        let owner = 1;
        let origin = Origin::signed(owner);
        let (asset_id,
             pre_image,
             anchor_id,
             (proofs, static_hashes, _),
             nft_data,
             _) = setup_mint::<Test>(origin.clone(), token_id);

        // Place document anchor into storage for verification
        let wrong_doc_root = <Test as frame_system::Trait>::Hashing::hash_of(&pre_image);
        assert_ok!( <anchor::Module<Test>>::commit(
            origin.clone(),
            pre_image.clone(),
            wrong_doc_root,
            // Proof does not matter here
            <Test as frame_system::Trait>::Hashing::hash_of(&0),
            crate::common::MS_PER_DAY + 1) );

        let (registry_id, token_id) = asset_id.destruct();

        // Mint token with document proof
        assert_err!(
            SUT::mint(origin,
                      owner,
                      registry_id,
                      token_id,
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
        let token_id = U256::one();
        let owner = 1;
        let origin = Origin::signed(owner);
        let (asset_id,
             pre_image,
             anchor_id,
             (proofs, static_hashes, doc_root),
             nft_data,
             _) = setup_mint::<Test>(origin.clone(), token_id);

        // Place document anchor into storage for verification
        assert_ok!( <anchor::Module<Test>>::commit(
            origin.clone(),
            pre_image,
            doc_root,
            // Proof does not matter here
            <Test as frame_system::Trait>::Hashing::hash_of(&0),
            crate::common::MS_PER_DAY + 1) );

        let (registry_id, token_id) = asset_id.destruct();

        // Mint token with document proof
        assert_ok!(
            SUT::mint(origin.clone(),
                      owner,
                      registry_id,
                      token_id,
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
                      registry_id,
                      token_id,
                      nft_data.clone(),
                      MintInfo {
                          anchor_id: anchor_id,
                          proofs: proofs,
                          static_hashes: static_hashes,
                      }),
            NftError::<Test>::AssetExists);
    });
}

#[test]
fn mint_fails_with_wrong_tokenid_in_proof() {
    new_test_ext().execute_with(|| {
        let token_id = U256::one();
        let owner = 1;
        let origin = Origin::signed(owner);
        let (asset_id,
             pre_image,
             anchor_id,
             (proofs, static_hashes, doc_root),
             nft_data,
             _) = setup_mint::<Test>(origin.clone(), token_id);

        // Place document anchor into storage for verification
        assert_ok!( <anchor::Module<Test>>::commit(
            origin.clone(),
            pre_image,
            doc_root,
            // Proof does not matter here
            <Test as frame_system::Trait>::Hashing::hash_of(&0),
            crate::common::MS_PER_DAY + 1) );

        let (registry_id, _) = asset_id.destruct();
        let token_id = U256::zero();

        // Mint token with document proof
        assert_err!(
            SUT::mint(origin,
                      owner,
                      registry_id,
                      token_id,
                      nft_data.clone(),
                      MintInfo {
                          anchor_id: anchor_id,
                          proofs: proofs,
                          static_hashes: static_hashes,
                      }),
            Error::<Test>::InvalidProofs);
    });
}
