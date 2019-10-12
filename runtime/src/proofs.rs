use rstd::vec::Vec;

#[derive(Debug)]
pub struct Proof {
    hash: [u8; 32],
    sorted_hashes: Vec<[u8; 32]>,
}

/// validates each proof and return true if all the proofs are valid
/// else returns false
pub fn validate_proofs(doc_root: [u8; 32], proofs: &Vec<Proof>) -> bool {
    if proofs.len() < 1 || doc_root.len() < 1 {
        return false;
    }

    let mut matches = Vec::new();
    matches.push(doc_root);
    return proofs
        .iter()
        .map(|proof| {
            validate_proof(
                &mut matches,
                proof.hash.clone(),
                proof.sorted_hashes.clone(),
            )
        })
        .fold(true, |acc, b| acc && b);
}

fn hash_of(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    let mut h: Vec<u8> = Vec::with_capacity(64);
    if a < b {
        h.extend_from_slice(&a);
        h.extend_from_slice(&b);
    } else {
        h.extend_from_slice(&b);
        h.extend_from_slice(&a);
    }

    runtime_io::blake2_256(&h)
}

fn validate_proof(matches: &mut Vec<[u8; 32]>, hash: [u8; 32], proofs: Vec<[u8; 32]>) -> bool {
    // if hash is already cached earlier
    if matches.contains(&hash) {
        return true;
    }

    let mut hash = hash.clone();
    for proof in proofs.into_iter() {
        matches.push(proof);
        hash = hash_of(hash, proof);
        if matches.contains(&hash) {
            return true;
        }
        matches.push(hash.clone())
    }

    false
}

// appends all the hashes from the proofs and returns keccak hash of the result.
pub fn bundled_hash(proofs: Vec<Proof>) -> [u8; 32] {
    let hash = proofs
        .into_iter()
        .fold(Vec::new(), |mut acc, proof: Proof| {
            acc.extend_from_slice(&proof.hash);
            acc
        });

    runtime_io::keccak_256(hash.as_slice())
}

#[cfg(test)]
mod tests {
    use crate::proofs::{bundled_hash, hash_of, validate_proof, validate_proofs, Proof};

    fn proof_from_hash(a: [u8; 32]) -> Proof {
        Proof {
            hash: a,
            sorted_hashes: Vec::new(),
        }
    }

    #[test]
    fn hash_of_a_lt_b() {
        let a = [
            85, 191, 116, 245, 55, 139, 29, 147, 139, 183, 161, 63, 60, 101, 130, 105, 30, 215,
            162, 223, 133, 233, 58, 181, 111, 161, 24, 186, 201, 162, 18, 68,
        ];
        let b = [
            126, 71, 93, 133, 114, 129, 33, 224, 177, 195, 218, 219, 37, 144, 248, 166, 154, 234,
            111, 197, 57, 209, 116, 232, 90, 189, 173, 122, 131, 190, 143, 142,
        ];
        let res = [
            29, 106, 103, 53, 85, 94, 151, 152, 97, 33, 199, 77, 199, 229, 218, 111, 251, 9, 138,
            235, 120, 71, 98, 105, 91, 212, 180, 209, 164, 91, 87, 156,
        ];
        let got = hash_of(a, b);
        assert!(res == got, "{:?} {:?}", res, got)
    }

    #[test]
    fn hash_of_a_ge_b() {
        let a = [
            195, 54, 245, 186, 28, 27, 161, 155, 121, 162, 87, 70, 124, 245, 203, 204, 222, 221,
            76, 181, 36, 224, 146, 47, 121, 48, 61, 76, 41, 196, 214, 202,
        ];
        let b = [
            28, 155, 171, 103, 166, 215, 230, 103, 16, 241, 86, 246, 149, 196, 131, 65, 159, 211,
            236, 57, 178, 89, 170, 125, 116, 181, 197, 170, 8, 84, 41, 159,
        ];
        let res = [
            237, 165, 215, 95, 110, 141, 136, 232, 17, 105, 160, 71, 23, 210, 172, 113, 170, 84,
            158, 210, 122, 74, 55, 7, 101, 217, 146, 206, 194, 114, 79, 169,
        ];
        let got = hash_of(a, b);
        assert!(res == got, "{:?} {:?}", res, got)
    }

    #[test]
    fn bundled_hash_with_leaves() {
        let proofs: Vec<Proof> = vec![
            proof_from_hash([
                103, 46, 60, 60, 148, 2, 8, 108, 29, 15, 111, 98, 88, 90, 56, 3, 57, 124, 5, 25,
                100, 82, 231, 99, 186, 115, 165, 102, 22, 245, 83, 147,
            ]),
            proof_from_hash([
                112, 102, 224, 155, 227, 136, 160, 106, 127, 252, 25, 95, 234, 206, 155, 3, 237,
                180, 242, 172, 240, 225, 85, 46, 125, 73, 42, 225, 214, 242, 239, 184,
            ]),
            proof_from_hash([
                131, 137, 170, 250, 176, 243, 90, 79, 242, 135, 64, 183, 249, 106, 200, 177, 96,
                105, 70, 38, 50, 221, 139, 175, 247, 161, 201, 31, 71, 169, 101, 114,
            ]),
            proof_from_hash([
                181, 110, 49, 204, 113, 201, 241, 253, 213, 177, 124, 217, 157, 68, 43, 8, 157,
                127, 218, 194, 90, 40, 153, 33, 125, 155, 10, 73, 20, 173, 89, 193,
            ]),
        ];

        let res = [
            188, 15, 195, 125, 35, 113, 141, 89, 32, 22, 122, 57, 68, 106, 224, 40, 255, 233, 239,
            61, 31, 123, 119, 181, 238, 145, 82, 93, 130, 187, 130, 12,
        ];
        let got = bundled_hash(proofs);
        assert!(res == got, "{:?} {:?}", res, got)
    }

    fn get_valid_proof() -> (Proof, [u8; 32]) {
        let proof = Proof {
            hash: [
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
            ],
            sorted_hashes: vec![
                [
                    113, 229, 58, 223, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
                    223, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
                ],
                [
                    133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
                    92, 232, 170, 46, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
                ],
                [
                    197, 248, 165, 165, 247, 119, 114, 231, 95, 114, 94, 16, 66, 142, 230, 184, 78,
                    203, 73, 104, 24, 82, 134, 154, 180, 129, 71, 223, 72, 31, 230, 15,
                ],
                [
                    50, 5, 28, 219, 118, 141, 222, 221, 133, 174, 178, 212, 71, 94, 64, 44, 80,
                    218, 29, 92, 77, 40, 241, 16, 126, 48, 119, 31, 6, 147, 224, 5,
                ],
            ],
        };

        let doc_root = [
            25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175, 70,
            161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
        ];

        (proof, doc_root)
    }

    fn get_invalid_proof() -> (Proof, [u8; 32]) {
        let proof = Proof {
            hash: [
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 20, 48, 97, 34, 3, 169, 157, 88, 159,
            ],
            sorted_hashes: vec![
                [
                    113, 229, 58, 22, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
                    223, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
                ],
                [
                    133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
                    92, 23, 170, 4, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
                ],
            ],
        };

        let doc_root = [
            25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175, 70,
            161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
        ];

        (proof, doc_root)
    }

    #[test]
    fn validate_proof_success() {
        let (proof, root) = get_valid_proof();
        let mut matches = vec![root];

        assert!(validate_proof(
            &mut matches,
            proof.hash,
            proof.sorted_hashes
        ))
    }

    #[test]
    fn validate_proof_failed() {
        let (proof, doc_root) = get_invalid_proof();
        let mut matches = vec![doc_root];

        assert!(!validate_proof(
            &mut matches,
            proof.hash,
            proof.sorted_hashes
        ))
    }

    #[test]
    fn validate_proof_no_proofs() {
        let proof = Proof {
            hash: [
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
            ],
            sorted_hashes: vec![],
        };

        let mut matches: Vec<[u8; 32]> = vec![[
            25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175, 70,
            161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
        ]];

        assert!(!validate_proof(
            &mut matches,
            proof.hash,
            proof.sorted_hashes
        ))
    }

    #[test]
    fn validate_proofs_success() {
        let (vp, doc_root) = get_valid_proof();
        let proofs = vec![vp];
        assert!(validate_proofs(doc_root, &proofs))
    }

    #[test]
    fn validate_proofs_failed() {
        let (vp, doc_root) = get_valid_proof();
        let (ivp, _) = get_invalid_proof();
        let proofs = vec![vp, ivp];
        assert!(!validate_proofs(doc_root, &proofs))
    }

    #[test]
    fn validate_proofs_no_proofs() {
        let (_, doc_root) = get_valid_proof();
        let proofs = vec![];
        assert!(!validate_proofs(doc_root, &proofs))
    }

    #[test]
    fn validate_proofs_no_doc_root() {
        let proofs = vec![];
        assert!(!validate_proofs([0; 32], &proofs))
    }
}
