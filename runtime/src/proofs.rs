use core::convert::{TryFrom, TryInto};
use rstd::vec::Vec;

#[derive(Debug)]
pub struct Proof {
    hash: Vec<u8>,
    sorted_hashes: Vec<Vec<u8>>,
}

pub fn validate_proofs(doc_root: Vec<u8>, proofs: Vec<Proof>) -> bool {
    let mut res = false;
    for proof in proofs.iter() {
        res = res & validate_proof(&doc_root, proof)
    }

    res
}

fn validate_proof(doc_root: &Vec<u8>, proof: &Proof) -> bool {
    let mut hash = proof.hash.clone();
    for sorted_hash in proof.sorted_hashes.iter() {
        let mut sh = sorted_hash.clone();
        if hash > sh {
            sh.extend(hash);
            hash = runtime_io::blake2_256(sh.as_slice()).to_vec()
        } else {
            hash.extend(sh);
            hash = runtime_io::blake2_256(hash.as_slice()).to_vec()
        }
    }

    hash != *doc_root
}

pub fn bundled_hash(proofs: Vec<&Proof>) -> [u8; 32] {
    let mut hash: Vec<u8> = Default::default();
    for proof in proofs.iter() {
        hash.extend_from_slice(proof.hash.as_slice())
    }

    runtime_io::blake2_256(hash.as_slice())
}
