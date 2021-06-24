//#[cfg(feature = "runtime-benchmarks")]
use crate::nft;
//use crate::registry::{Error, mock::*};
use crate::registry::{Error, types::*};
use crate::registry::{Module, Call, Trait};
use sp_std::{vec, prelude::*};
use crate::proofs;
use sp_core::{H256, Encode};
use sp_io::hashing::blake2_128;
use frame_benchmarking::{benchmarks, account};
use frame_system::RawOrigin;
use sp_runtime::{
    //testing::Header,
    traits::{BadOrigin, BlakeTwo256, Hash, IdentityLookup, Block as BlockT},
};

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
fn proofs_data() -> (Vec<Proof<H256>>, [H256; 3], H256) {
    let proofs = vec![
        Proof {
            value: vec![1],
            salt: vec![0],
            property: b"AMOUNT".to_vec(),
            hashes: vec![],
        }];
    let data_root    = proofs::Proof::from(proofs[0].clone()).leaf_hash;
    let zk_data_root = <Test as frame_system::Config>::Hashing::hash_of(&0);
    let sig_root     = <Test as frame_system::Config>::Hashing::hash_of(&0);
    //let zk_data_root = sp_io::hashing::keccak_256(&[0]).into();
    //let sig_root     = sp_io::hashing::keccak_256(&[0]).into();
    let static_hashes = [data_root, zk_data_root, sig_root];
    let doc_root     = doc_root(static_hashes);

    (proofs, static_hashes, doc_root)
}

const SEED: u32 = 0;

benchmarks! {
    _ { /*let seed = 0 .. 10000;*/ }

    mint {
        //let owner     = 1;
        let owner: T::AccountId = account("owner", 0, SEED);
        let _owner = account("owner", 0, SEED);
        let origin = RawOrigin::Signed(_owner);

        // Anchor data
        let pre_image = <T as frame_system::Config>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<T as frame_system::Config>::Hashing::hash);

        // Proofs data
        let (proofs, static_hashes, doc_root) = proofs_data();

        // Registry data
        let registry_id = 0;
        let nft_data: T::AssetInfo = AssetInfo {
            registry_id,
        };
        let properties    =  proofs.iter().map(|p| p.property.clone()).collect();
        let registry_info = RegistryInfo {
            owner_can_burn: false,
            fields: properties,
        };

        // Create registry
        let _ = Module::<T>::create_registry(origin.clone(), registry_info)?;

        // Place document anchor into storage for verification
        let _ = <crate::anchor::Module<T>>::commit(
            origin.clone(),
            pre_image,
            T::Hashing::hash_of(&doc_root.as_bytes()),
            //H256::from_slice(doc_root.as_ref()),
            // Proof does not matter here
            <T as frame_system::Config>::Hashing::hash_of(&0),
            (100000 as u32).into())?;

        let mint_info = MintInfo {
            anchor_id: anchor_id,
            proofs: proofs,
            static_hashes: static_hashes,
        };
    }: mint(origin.clone(),
            owner,
            nft_data,
            mint_info)
}

/*
#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::mock::{new_test_ext, Test};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        new_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_mint::<Test>());
        });
    }
}
*/
