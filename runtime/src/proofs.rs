use rstd::vec::Vec;

#[derive(Debug)]
pub struct Proof {
    hash: Vec<u8>,
    sorted_hashes: Vec<Vec<u8>>,
}

/// validates each proof and return true if all the proofs are valid
/// else returns false
pub fn validate_proofs(doc_root: Vec<u8>, proofs: &Vec<Proof>) -> bool {
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

fn hash_of(a: &mut Vec<u8>, b: &mut Vec<u8>) -> Vec<u8> {
    let mut h: Vec<u8> = Vec::new();
    if a < b {
        h.append(a);
        h.append(b);
    } else {
        h.append(b);
        h.append(a);
    }

    runtime_io::blake2_256(h.as_slice()).to_vec()
}

fn validate_proof(matches: &mut Vec<Vec<u8>>, hash: Vec<u8>, proofs: Vec<Vec<u8>>) -> bool {
    // if hash is already cached earlier
    if matches.contains(&hash) {
        return true;
    }

    let mut hash = hash.clone();
    for proof in proofs.iter() {
        matches.push(proof.clone());
        hash = hash_of(&mut hash, &mut proof.clone());
        if matches.contains(&hash) {
            return true;
        }
        matches.push(hash.clone())
    }

    false
}

// appends all the hashes from the proofs and returns keccak hash of the result.
pub fn bundled_hash(proofs: Vec<Proof>) -> [u8; 32] {
    let hash = proofs.iter().fold(Vec::new(), |mut acc, proof: &Proof| {
        acc.extend_from_slice(proof.hash.as_slice());
        acc
    });

    runtime_io::keccak_256(hash.as_slice())
}

#[cfg(test)]
mod tests {
    use crate::proofs::{bundled_hash, hash_of, validate_proof, validate_proofs, Proof};

    fn proof_from_hash(a: Vec<u8>) -> Proof {
        Proof {
            hash: a,
            sorted_hashes: Vec::new(),
        }
    }

    #[test]
    fn hash_of_a_lt_b() {
        let mut a: Vec<u8> = vec![72, 101, 108, 108, 111];
        let mut b: Vec<u8> = vec![119, 111, 114, 108, 100];
        let res: Vec<u8> = vec![
            111, 230, 200, 49, 169, 158, 202, 251, 252, 28, 183, 206, 5, 50, 171, 126, 148, 32,
            117, 151, 138, 198, 18, 188, 204, 182, 144, 206, 162, 138, 198, 207,
        ];
        let got = hash_of(&mut a, &mut b);
        assert!(res == got, "{:?} {:?}", res, got)
    }

    #[test]
    fn hash_of_a_ge_b() {
        let mut a: Vec<u8> = vec![119, 111, 114, 108, 100];
        let mut b: Vec<u8> = vec![72, 101, 108, 108, 111];
        let res: Vec<u8> = vec![
            111, 230, 200, 49, 169, 158, 202, 251, 252, 28, 183, 206, 5, 50, 171, 126, 148, 32,
            117, 151, 138, 198, 18, 188, 204, 182, 144, 206, 162, 138, 198, 207,
        ];
        let got = hash_of(&mut a, &mut b);
        assert!(res == got, "{:?} {:?}", res, got)
    }

    #[test]
    fn bundled_hash_with_leaves() {
        let proofs: Vec<Proof> = vec![
            proof_from_hash(vec![
                103, 46, 60, 60, 148, 2, 8, 108, 29, 15, 111, 98, 88, 90, 56, 3, 57, 124, 5, 25,
                100, 82, 231, 99, 186, 115, 165, 102, 22, 245, 83, 147,
            ]),
            proof_from_hash(vec![
                112, 102, 224, 155, 227, 136, 160, 106, 127, 252, 25, 95, 234, 206, 155, 3, 237,
                180, 242, 172, 240, 225, 85, 46, 125, 73, 42, 225, 214, 242, 239, 184,
            ]),
            proof_from_hash(vec![
                131, 137, 170, 250, 176, 243, 90, 79, 242, 135, 64, 183, 249, 106, 200, 177, 96,
                105, 70, 38, 50, 221, 139, 175, 247, 161, 201, 31, 71, 169, 101, 114,
            ]),
            proof_from_hash(vec![
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

    fn get_valid_proof() -> (Proof, Vec<u8>) {
        let proof = Proof {
            hash: vec![
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
            ],
            sorted_hashes: vec![
                vec![
                    113, 229, 58, 223, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
                    223, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
                ],
                vec![
                    133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
                    92, 232, 170, 46, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
                ],
                vec![
                    197, 248, 165, 165, 247, 119, 114, 231, 95, 114, 94, 16, 66, 142, 230, 184, 78,
                    203, 73, 104, 24, 82, 134, 154, 180, 129, 71, 223, 72, 31, 230, 15,
                ],
                vec![
                    50, 5, 28, 219, 118, 141, 222, 221, 133, 174, 178, 212, 71, 94, 64, 44, 80,
                    218, 29, 92, 77, 40, 241, 16, 126, 48, 119, 31, 6, 147, 224, 5,
                ],
            ],
        };

        let doc_root = vec![
            25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175, 70,
            161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
        ];

        (proof, doc_root)
    }

    fn get_invalid_proof() -> (Proof, Vec<u8>) {
        let proof = Proof {
            hash: vec![
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 20, 48, 97, 34, 3, 169, 157, 88, 159,
            ],
            sorted_hashes: vec![
                vec![
                    113, 229, 58, 22, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
                    223, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
                ],
                vec![
                    133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
                    92, 23, 170, 4, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
                ],
            ],
        };

        let doc_root = vec![
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
            hash: vec![
                1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
                37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
            ],
            sorted_hashes: vec![],
        };

        let mut matches: Vec<Vec<u8>> = vec![vec![
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
        assert!(!validate_proofs(vec![], &proofs))
    }
}
