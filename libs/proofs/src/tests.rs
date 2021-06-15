mod tests {
	use crate::{
		hashing::bundled_hash,
		mock::{get_invalid_proof, get_valid_proof, BundleHasher, ProofVerifier},
	};
	use crate::{Proof, Verifier};

	use sp_core::H256;

	#[test]
	fn bundled_hash_with_leaves() {
		let proofs: Vec<H256> = vec![
			[
				103, 46, 60, 60, 148, 2, 8, 108, 29, 15, 111, 98, 88, 90, 56, 3, 57, 124, 5, 25,
				100, 82, 231, 99, 186, 115, 165, 102, 22, 245, 83, 147,
			]
			.into(),
			[
				112, 102, 224, 155, 227, 136, 160, 106, 127, 252, 25, 95, 234, 206, 155, 3, 237,
				180, 242, 172, 240, 225, 85, 46, 125, 73, 42, 225, 214, 242, 239, 184,
			]
			.into(),
			[
				131, 137, 170, 250, 176, 243, 90, 79, 242, 135, 64, 183, 249, 106, 200, 177, 96,
				105, 70, 38, 50, 221, 139, 175, 247, 161, 201, 31, 71, 169, 101, 114,
			]
			.into(),
			[
				181, 110, 49, 204, 113, 201, 241, 253, 213, 177, 124, 217, 157, 68, 43, 8, 157,
				127, 218, 194, 90, 40, 153, 33, 125, 155, 10, 73, 20, 173, 89, 193,
			]
			.into(),
		];

		let deposit_address = [
			75, 151, 92, 119, 170, 193, 75, 255, 44, 88, 202, 225, 39, 220, 51, 9, 230, 2, 121, 129,
		];

		let res: H256 = [
			92, 231, 93, 51, 106, 224, 159, 91, 206, 250, 124, 26, 16, 236, 141, 56, 42, 126, 225,
			64, 28, 191, 37, 51, 131, 63, 224, 233, 24, 207, 211, 182,
		]
		.into();
		let got = bundled_hash::<BundleHasher>(proofs, deposit_address);
		assert_eq!(res, got, "must be equal");
	}

	#[test]
	fn validate_proof_success() {
		let (proof, root) = get_valid_proof();
		let pv = ProofVerifier;
		assert!(pv.verify_proof(root, &proof))
	}

	#[test]
	fn validate_proof_failed() {
		let (proof, root) = get_invalid_proof();
		let pv = ProofVerifier;
		assert!(!pv.verify_proof(root, &proof));
	}

	#[test]
	fn validate_proof_no_proofs() {
		let proof: Proof<H256> = Proof {
			leaf_hash: [
				1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
				37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
			]
			.into(),
			sorted_hashes: vec![],
		};

		let doc_root: H256 = [
			25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175, 70,
			161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
		]
		.into();

		let pv = ProofVerifier;
		assert!(!pv.verify_proof(doc_root, &proof));
	}

	#[test]
	fn validate_proofs_success() {
		let (vp1, doc_root) = get_valid_proof();
		let (vp2, _) = get_valid_proof();
		let proofs = vec![vp1, vp2];
		let pv = ProofVerifier;
		assert!(pv.verify_proofs(doc_root, &proofs));
	}

	#[test]
	fn validate_proofs_failed() {
		let (vp, doc_root) = get_valid_proof();
		let (ivp, _) = get_invalid_proof();
		let proofs = vec![vp, ivp];
		let pv = ProofVerifier;
		assert!(!pv.verify_proofs(doc_root, &proofs));
	}

	#[test]
	fn validate_proofs_no_proofs() {
		let (_, doc_root) = get_valid_proof();
		let proofs = vec![];
		let pv = ProofVerifier;
		assert!(!pv.verify_proofs(doc_root, &proofs));
	}
}
