//use rstd::collections::HashMap;
use rstd::vec::Vec;

#[derive(Debug)]
pub struct Proof {
    hash: Vec<u8>,
    sorted_hashes: Vec<Vec<u8>>,
}

/// validates each proof and return true if all the proofs are valid
/// else returns false
//pub fn validate_proofs(doc_root: Vec<u8>, proofs: &Vec<Proof>) -> bool {
//    let mut matches = HashMap::new();
//    matches.insert(doc_root, ());
//    return proofs
//        .iter()
//        .map(|proof| validate_proof(&mut matches, proof.hash, proof.sorted_hashes))
//        .fold(false, |acc, b| acc && b);
//}

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

//fn validate_proof(matches: &mut HashMap<Vec<u8>, ()>, hash: Vec<u8>, proofs: Vec<Vec<u8>>) -> bool {
//    // if hash is already cached earlier
//    let mut hash = hash.clone();
//    if matches.contains_key(hash.as_slice()) {
//        return true;
//    }
//
//    for proof in proofs.iter() {
//        matches.insert(proof.clone(), ());
//        hash = hash_of(&mut hash, &mut proof.clone());
//        if matches.contains_key(hash.as_slice()) {
//            return true;
//        }
//        matches.insert(hash, ())
//    }
//
//    false
//}

pub fn bundled_hash(proofs: Vec<Proof>) -> [u8; 32] {
    let hash = proofs.iter().fold(Vec::new(), |mut acc, proof: &Proof| {
        acc.extend_from_slice(proof.hash.as_slice());
        acc
    });

    runtime_io::keccak_256(hash.as_slice())
}

#[cfg(test)]
mod tests {
    use crate::proofs::{bundled_hash, hash_of, Proof};

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
}
