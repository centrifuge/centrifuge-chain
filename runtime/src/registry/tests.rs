use crate::{Error, mock::*};
use crate::proofs::Proof;
use sp_core::{H256, Encode};
use frame_support::{assert_ok, Hashable};
use sp_runtime::{
    testing::Header,
    traits::{BadOrigin, BlakeTwo256, Hash, IdentityLookup, Block as BlockT},
};
use sp_core::hashing::blake2_128;
use super::*;


fn get_valid_proof() -> (Proof, sp_core::H256, [H256; 3]) {
   let proof = Proof::new(
       [
           1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
           37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
       ]
       .into(),
       vec![
           [
               113, 229, 58, 223, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
               223, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
           ]
           .into(),
           [
               133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
               92, 232, 170, 46, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
           ]
           .into(),
           [
               197, 248, 165, 165, 247, 119, 114, 231, 95, 114, 94, 16, 66, 142, 230, 184, 78,
               203, 73, 104, 24, 82, 134, 154, 180, 129, 71, 223, 72, 31, 230, 15,
           ]
           .into(),
           [
               50, 5, 28, 219, 118, 141, 222, 221, 133, 174, 178, 212, 71, 94, 64, 44, 80,
               218, 29, 92, 77, 40, 241, 16, 126, 48, 119, 31, 6, 147, 224, 5,
           ]
           .into(),
       ],
   );
   let doc_root: H256 = [
       48, 123, 58, 192, 8, 62, 20, 55, 99, 52, 37, 73, 174, 123, 214, 104, 37, 41, 189, 170,
       205, 80, 158, 136, 224, 128, 128, 89, 55, 240, 32, 234,
   ]
   .into();

   let static_proofs: [H256; 3] = [
       [
           25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175,
           70, 161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
       ]
       .into(),
       [
           61, 164, 199, 22, 164, 251, 58, 14, 67, 56, 242, 60, 86, 203, 128, 203, 138, 129,
           237, 7, 29, 7, 39, 58, 250, 42, 14, 53, 241, 108, 187, 74,
       ]
       .into(),
       [
           70, 124, 133, 120, 103, 45, 94, 174, 176, 18, 151, 243, 104, 120, 12, 54, 217, 189,
           59, 222, 109, 64, 136, 203, 56, 136, 159, 115, 96, 101, 2, 185,
       ]
       .into(),
   ];

   (proof, doc_root, static_proofs)
}


#[test]
fn mint_with_valid_proofs_works() {
    new_test_ext().execute_with(|| {
        let owner     = 1;
        let origin    = Origin::signed(1);
        let (pf, doc_root, static_proofs) = get_valid_proof();
        let pre_image = <Test as frame_system::Trait>::Hashing::hash_of(&0);
        let anchor_id = (pre_image).using_encoded(<Test as frame_system::Trait>::Hashing::hash);
        let fields = vec![vec![0], vec![1]];
        let values = vec![vec![2], vec![3]];

        let registry_id = 0;
        let nft_data = AssetInfo {
            registry_id,
        };
        let registry_info = RegistryInfo {
            owner_can_burn: false,
            fields: fields,
        };

        // Starts with no Nfts
        assert_eq!(<pallet_nft::Module<Test>>::total(), 0);
        assert_eq!(<pallet_nft::Module<Test>>::total_for_account(owner), 0);

        // Create registry
        assert_ok!(
            SUT::create_registry(origin.clone(), registry_info)
        );

        // Place document anchor into storage for verification
        assert_ok!(SUT::tmp_set_anchor(origin.clone(), anchor_id, doc_root));

        // Mint token with document proof
        assert_ok!(
            SUT::mint(origin,
                      owner,
                      nft_data.clone(),
                      MintInfo {
                          anchor_id: anchor_id,
                          proofs: vec![pf],
                          values: values,
                      }));

        // Nft registered to owner
        assert_eq!(
            <pallet_nft::Module<Test>>::account_for_commodity::<H256>(<Test as frame_system::Trait>::Hashing::hash_of(&nft_data)),
            owner
        );

        // Total Nfts did increase
        assert_eq!(<pallet_nft::Module<Test>>::total(), 1);
        assert_eq!(<pallet_nft::Module<Test>>::total_for_account(owner), 1);
    });
}

#[test]
fn create_registry_works() {
    new_test_ext().execute_with(|| {
        let origin = Origin::signed(1);
    });
}
