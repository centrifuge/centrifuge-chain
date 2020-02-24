use codec::{Decode, Encode};
use sp_core::H256;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(not(feature = "std"), derive(RuntimeDebug))]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Proof {
    leaf_hash: H256,
    sorted_hashes: Vec<H256>,
}

impl Proof {
    pub fn new(hash: H256, sorted_hashes: Vec<H256>) -> Self {
        Self {
            leaf_hash: hash,
            sorted_hashes,
        }
    }
}

/// Validates each proof and return true if all the proofs are valid else returns false
///
/// This is an optimized Merkle proof checker. It caches all valid leaves in an array called
/// matches. If a proof is validated, all the intermediate hashes will be added to the array.
/// When validating a subsequent proof, that proof will stop being validated as soon as a hash
/// has been computed that has been a computed hash in a previously validated proof.
///
/// When submitting a list of proofs, the client can thus choose to chop of all the already proven
/// nodes when submitting multiple proofs.
///
/// matches: matches will have a pre computed hashes provided by the client and document root of the
/// reference anchor. static proofs are used to computed the pre computed hashes and the result is
/// checked against document root provided.
pub fn validate_proofs(doc_root: H256, proofs: &Vec<Proof>, static_proofs: [H256; 3]) -> bool {
    if proofs.len() < 1 {
        return false;
    }

    let (valid, mut matches) = pre_matches(static_proofs, doc_root);
    if !valid {
        return false;
    }

    return proofs
        .iter()
        .map(|proof| validate_proof(&mut matches, proof.leaf_hash, proof.sorted_hashes.clone()))
        .fold(true, |acc, b| acc && b);
}

// computes blake2 256 sorted hash of the a and b
// if a < b: blake256(a+b)
// else: blake256(b+a)
fn sort_hash_of(a: H256, b: H256) -> H256 {
    let mut h: Vec<u8> = Vec::with_capacity(64);
    if a < b {
        h.extend_from_slice(&a[..]);
        h.extend_from_slice(&b[..]);
    } else {
        h.extend_from_slice(&b[..]);
        h.extend_from_slice(&a[..]);
    }

    sp_io::hashing::blake2_256(&h).into()
}

// computes blake2 256 hash of the a + b
fn hash_of(a: H256, b: H256) -> H256 {
    let mut h: Vec<u8> = Vec::with_capacity(64);
    h.extend_from_slice(&a[..]);
    h.extend_from_slice(&b[..]);
    sp_io::hashing::blake2_256(&h).into()
}

// validates the proof by computing a sorted hash of the provided proofs with hash as initial value.
// each calculated hash is memoized.
// Validation stops as soon as the any computed hash is found in the matches.
// if no computed hash is found in the matches, validation fails.
fn validate_proof(matches: &mut Vec<H256>, hash: H256, proofs: Vec<H256>) -> bool {
    // if hash is already cached earlier
    if matches.contains(&hash) {
        return true;
    }

    let mut hash = hash;
    for proof in proofs.into_iter() {
        matches.push(proof);
        hash = sort_hash_of(hash, proof);
        if matches.contains(&hash) {
            return true;
        }
        matches.push(hash)
    }

    false
}

// pre_matches takes 3 static proofs and calculate document root.
// the calculated document root is then compared with the document root that is passed.
// if the calculated document root matches, returns true and array of precomputed hashes
// precomputed hashes are used while validating the proofs.
//
//
// Computing Document Root:
//                      DocumentRoot
//                      /          \
//          Signing Root            Signature Root
//          /          \
//   data root 1     data root 2
fn pre_matches(static_proofs: [H256; 3], doc_root: H256) -> (bool, Vec<H256>) {
    let mut matches = Vec::new();
    let basic_data_root = static_proofs[0];
    let zk_data_root = static_proofs[1];
    let signature_root = static_proofs[2];
    matches.push(basic_data_root);
    matches.push(zk_data_root);
    let signing_root = hash_of(basic_data_root, zk_data_root);
    matches.push(signing_root);
    matches.push(signature_root);
    let calc_doc_root = hash_of(signing_root, signature_root);
    matches.push(calc_doc_root);
    (calc_doc_root == doc_root, matches)
}

// appends deposit_address and all the hashes from the proofs and returns keccak hash of the result.
pub fn bundled_hash(proofs: Vec<Proof>, deposit_address: [u8; 20]) -> H256 {
    let hash = proofs
        .into_iter()
        .fold(deposit_address.to_vec(), |mut acc, proof: Proof| {
            acc.extend_from_slice(&proof.leaf_hash[..]);
            acc
        });

    sp_io::hashing::keccak_256(hash.as_slice()).into()
}

#[cfg(test)]
mod tests {
    use crate::proofs::{
        bundled_hash, pre_matches, sort_hash_of, validate_proof, validate_proofs, Proof,
    };
    use sp_core::H256;

    fn proof_from_hash(a: H256) -> Proof {
        Proof {
            leaf_hash: a,
            sorted_hashes: Vec::new(),
        }
    }

    #[test]
    fn hash_of_a_lt_b() {
        let a: H256 = [
            85, 191, 116, 245, 55, 139, 29, 147, 139, 183, 161, 63, 60, 101, 130, 105, 30, 215,
            162, 223, 133, 233, 58, 181, 111, 161, 24, 186, 201, 162, 18, 68,
        ]
        .into();
        let b: H256 = [
            126, 71, 93, 133, 114, 129, 33, 224, 177, 195, 218, 219, 37, 144, 248, 166, 154, 234,
            111, 197, 57, 209, 116, 232, 90, 189, 173, 122, 131, 190, 143, 142,
        ]
        .into();
        let res: H256 = [
            29, 106, 103, 53, 85, 94, 151, 152, 97, 33, 199, 77, 199, 229, 218, 111, 251, 9, 138,
            235, 120, 71, 98, 105, 91, 212, 180, 209, 164, 91, 87, 156,
        ]
        .into();
        let got = sort_hash_of(a, b);
        assert!(res == got, "{:?} {:?}", res, got)
    }

    #[test]
    fn hash_of_a_ge_b() {
        let a: H256 = [
            195, 54, 245, 186, 28, 27, 161, 155, 121, 162, 87, 70, 124, 245, 203, 204, 222, 221,
            76, 181, 36, 224, 146, 47, 121, 48, 61, 76, 41, 196, 214, 202,
        ]
        .into();
        let b: H256 = [
            28, 155, 171, 103, 166, 215, 230, 103, 16, 241, 86, 246, 149, 196, 131, 65, 159, 211,
            236, 57, 178, 89, 170, 125, 116, 181, 197, 170, 8, 84, 41, 159,
        ]
        .into();
        let res: H256 = [
            237, 165, 215, 95, 110, 141, 136, 232, 17, 105, 160, 71, 23, 210, 172, 113, 170, 84,
            158, 210, 122, 74, 55, 7, 101, 217, 146, 206, 194, 114, 79, 169,
        ]
        .into();
        let got = sort_hash_of(a, b);
        assert!(res == got, "{:?} {:?}", res, got)
    }

    #[test]
    fn bundled_hash_with_leaves() {
        let proofs: Vec<Proof> = vec![
            proof_from_hash(
                [
                    103, 46, 60, 60, 148, 2, 8, 108, 29, 15, 111, 98, 88, 90, 56, 3, 57, 124, 5,
                    25, 100, 82, 231, 99, 186, 115, 165, 102, 22, 245, 83, 147,
                ]
                .into(),
            ),
            proof_from_hash(
                [
                    112, 102, 224, 155, 227, 136, 160, 106, 127, 252, 25, 95, 234, 206, 155, 3,
                    237, 180, 242, 172, 240, 225, 85, 46, 125, 73, 42, 225, 214, 242, 239, 184,
                ]
                .into(),
            ),
            proof_from_hash(
                [
                    131, 137, 170, 250, 176, 243, 90, 79, 242, 135, 64, 183, 249, 106, 200, 177,
                    96, 105, 70, 38, 50, 221, 139, 175, 247, 161, 201, 31, 71, 169, 101, 114,
                ]
                .into(),
            ),
            proof_from_hash(
                [
                    181, 110, 49, 204, 113, 201, 241, 253, 213, 177, 124, 217, 157, 68, 43, 8, 157,
                    127, 218, 194, 90, 40, 153, 33, 125, 155, 10, 73, 20, 173, 89, 193,
                ]
                .into(),
            ),
        ];

        let deposit_address = [
            75, 151, 92, 119, 170, 193, 75, 255, 44, 88, 202, 225, 39, 220, 51, 9, 230, 2, 121, 129,
        ];

        let res: H256 = [
            92, 231, 93, 51, 106, 224, 159, 91, 206, 250, 124, 26, 16, 236, 141, 56, 42, 126, 225,
            64, 28, 191, 37, 51, 131, 63, 224, 233, 24, 207, 211, 182,
        ]
        .into();
        let got = bundled_hash(proofs, deposit_address);
        assert!(res == got, "{:?} {:?}", res, got)
    }

    fn get_valid_proof() -> (Proof, H256, [H256; 3]) {
        let proof = Proof {
            leaf_hash: [
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
            ]
            .into(),
            sorted_hashes: vec![
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
        };

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

    fn get_invalid_proof() -> (Proof, H256) {
        let proof = Proof {
            leaf_hash: [
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 20, 48, 97, 34, 3, 169, 157, 88, 159,
            ]
            .into(),
            sorted_hashes: vec![
                [
                    113, 229, 58, 22, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
                    223, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
                ]
                .into(),
                [
                    133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
                    92, 23, 170, 4, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
                ]
                .into(),
            ],
        };

        let doc_root: H256 = [
            25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175, 70,
            161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
        ]
        .into();

        (proof, doc_root)
    }

    #[test]
    fn validate_proof_success() {
        let (proof, root, static_proofs) = get_valid_proof();
        let (_, mut matches) = pre_matches(static_proofs, root);
        assert!(validate_proof(
            &mut matches,
            proof.leaf_hash,
            proof.sorted_hashes
        ))
    }

    #[test]
    fn validate_proof_failed() {
        let (proof, doc_root) = get_invalid_proof();
        let mut matches = vec![doc_root];

        assert!(!validate_proof(
            &mut matches,
            proof.leaf_hash,
            proof.sorted_hashes
        ))
    }

    #[test]
    fn validate_proof_no_proofs() {
        let proof = Proof {
            leaf_hash: [
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
            ]
            .into(),
            sorted_hashes: vec![],
        };

        let mut matches: Vec<H256> = vec![[
            25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175, 70,
            161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
        ]
        .into()];

        assert!(!validate_proof(
            &mut matches,
            proof.leaf_hash,
            proof.sorted_hashes
        ))
    }

    #[test]
    fn validate_proofs_success() {
        let (vp1, doc_root, static_proofs) = get_valid_proof();
        let (vp2, _, _) = get_valid_proof();
        let proofs = vec![vp1, vp2];
        assert!(validate_proofs(doc_root, &proofs, static_proofs))
    }

    #[test]
    fn validate_proofs_failed() {
        let (vp, doc_root, static_proofs) = get_valid_proof();
        let (ivp, _) = get_invalid_proof();
        let proofs = vec![vp, ivp];
        assert!(!validate_proofs(doc_root, &proofs, static_proofs))
    }

    #[test]
    fn validate_proofs_no_proofs() {
        let (_, doc_root, static_proofs) = get_valid_proof();
        let proofs = vec![];
        assert!(!validate_proofs(doc_root, &proofs, static_proofs))
    }
}
