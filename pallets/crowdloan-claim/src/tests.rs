// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

//! Crowdloan claim pallet's unit test cases

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use frame_support::{assert_noop, assert_ok, traits::VestingSchedule};
use hex;
use sp_core::H256;
use sp_runtime::Perbill;
use sp_std::str::FromStr;

use crate as pallet_crowdloan_claim;
use crate::{mock::*, Error as CrowdloanClaimError, *};

struct Contributor {
	proof: proofs::Proof<H256>,
	signature: MultiSignature,
	parachain_account: u64,
	relaychain_account: AccountId32,
	contribution: u64,
}

fn get_root() -> H256 {
	let amount = 4000000000000000u64;
	let contributor =
		AccountId32::from_str("0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d")
			.unwrap();

	let mut v: Vec<u8> = contributor.encode();
	v.extend(amount.encode());
	let leaf_hash = <Runtime as frame_system::Config>::Hashing::hash(&v);

	// 10-leaf tree
	let leaf_hash_0: H256 = [0; 32].into();
	let leaf_hash_1: H256 = [1; 32].into();
	let leaf_hash_2: H256 = leaf_hash;
	let leaf_hash_3: H256 = [3; 32].into();
	let leaf_hash_4: H256 = [4; 32].into();
	let leaf_hash_5: H256 = [5; 32].into();
	let leaf_hash_6: H256 = [6; 32].into();
	let leaf_hash_7: H256 = [7; 32].into();
	let leaf_hash_8: H256 = [8; 32].into();
	let leaf_hash_9: H256 = [9; 32].into();
	let node_0 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_0,
		leaf_hash_1,
	);
	let node_1 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_2,
		leaf_hash_3,
	);
	let node_2 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_4,
		leaf_hash_5,
	);
	let node_3 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_6,
		leaf_hash_7,
	);
	let node_4 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_8,
		leaf_hash_9,
	);
	let node_00 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_0, node_1,
	);
	let node_01 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_2, node_3,
	);
	let node_000 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_00, node_01,
	);

	proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_000, node_4,
	)
}

fn get_root_for_ext_sig() -> H256 {
	let amount = 4000000000000000u64;
	let contributor: AccountId32 = AccountId32::from([
		202, 13, 159, 82, 100, 222, 166, 237, 52, 113, 173, 161, 100, 206, 112, 67, 188, 178, 135,
		53, 61, 178, 143, 121, 157, 182, 189, 207, 59, 166, 7, 92,
	]);

	let mut v: Vec<u8> = contributor.encode();
	v.extend(amount.encode());
	let leaf_hash = <Runtime as frame_system::Config>::Hashing::hash(&v);

	// 10-leaf tree
	let leaf_hash_0: H256 = [0; 32].into();
	let leaf_hash_1: H256 = [1; 32].into();
	let leaf_hash_2: H256 = leaf_hash;
	let leaf_hash_3: H256 = [3; 32].into();
	let leaf_hash_4: H256 = [4; 32].into();
	let leaf_hash_5: H256 = [5; 32].into();
	let leaf_hash_6: H256 = [6; 32].into();
	let leaf_hash_7: H256 = [7; 32].into();
	let leaf_hash_8: H256 = [8; 32].into();
	let leaf_hash_9: H256 = [9; 32].into();
	let node_0 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_0,
		leaf_hash_1,
	);
	let node_1 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_2,
		leaf_hash_3,
	);
	let node_2 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_4,
		leaf_hash_5,
	);
	let node_3 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_6,
		leaf_hash_7,
	);
	let node_4 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_8,
		leaf_hash_9,
	);
	let node_00 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_0, node_1,
	);
	let node_01 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_2, node_3,
	);
	let node_000 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_00, node_01,
	);

	proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_000, node_4,
	)
}

fn get_contributor_ext_sig() -> Contributor {
	let amount = 4000000000000000u64;
	let contributor: AccountId32 = AccountId32::from([
		202, 13, 159, 82, 100, 222, 166, 237, 52, 113, 173, 161, 100, 206, 112, 67, 188, 178, 135,
		53, 61, 178, 143, 121, 157, 182, 189, 207, 59, 166, 7, 92,
	]);

	let mut v: Vec<u8> = contributor.encode();
	v.extend(amount.encode());
	let leaf_hash = <Runtime as frame_system::Config>::Hashing::hash(&v);

	// 10-leaf tree
	let mut sorted_hashed: Vec<H256> = Vec::new();

	let leaf_hash_0: H256 = [0; 32].into();
	let leaf_hash_1: H256 = [1; 32].into();
	let leaf_hash_3: H256 = [3; 32].into();
	let leaf_hash_4: H256 = [4; 32].into();
	let leaf_hash_5: H256 = [5; 32].into();
	let leaf_hash_6: H256 = [6; 32].into();
	let leaf_hash_7: H256 = [7; 32].into();
	let leaf_hash_8: H256 = [8; 32].into();
	let leaf_hash_9: H256 = [9; 32].into();
	let node_0 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_0,
		leaf_hash_1,
	);
	let node_2 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_4,
		leaf_hash_5,
	);
	let node_3 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_6,
		leaf_hash_7,
	);
	let node_4 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_8,
		leaf_hash_9,
	);
	let node_01 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_2, node_3,
	);

	sorted_hashed.push(leaf_hash_3);
	sorted_hashed.push(node_0);
	sorted_hashed.push(node_01);
	sorted_hashed.push(node_4);

	// This signature is generate with the logic of: "Bytes" + Data + "Bytes".
	// Reason to test this is:
	let signature: [u8; 64] = [
		94, 215, 90, 147, 120, 40, 201, 58, 58, 69, 247, 113, 19, 18, 122, 200, 50, 194, 125, 133,
		72, 241, 154, 88, 122, 223, 138, 245, 237, 158, 72, 76, 144, 61, 44, 104, 25, 209, 72, 79,
		252, 106, 132, 44, 129, 133, 85, 45, 243, 53, 60, 167, 219, 238, 167, 89, 38, 204, 44, 245,
		99, 56, 31, 130,
	];

	Contributor {
		proof: proofs::Proof::new(leaf_hash, sorted_hashed),
		signature: MultiSignature::Sr25519(sp_core::sr25519::Signature(signature)),
		parachain_account: 1,
		relaychain_account: contributor,
		contribution: amount,
	}
}

fn get_contributor() -> Contributor {
	let amount = 4000000000000000u64;
	let contributor =
		AccountId32::from_str("0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d")
			.unwrap();

	let mut v: Vec<u8> = contributor.encode();
	v.extend(amount.encode());
	let leaf_hash = <Runtime as frame_system::Config>::Hashing::hash(&v);

	// 10-leaf tree
	let mut sorted_hashed: Vec<H256> = Vec::new();

	let leaf_hash_0: H256 = [0; 32].into();
	let leaf_hash_1: H256 = [1; 32].into();
	let leaf_hash_3: H256 = [3; 32].into();
	let leaf_hash_4: H256 = [4; 32].into();
	let leaf_hash_5: H256 = [5; 32].into();
	let leaf_hash_6: H256 = [6; 32].into();
	let leaf_hash_7: H256 = [7; 32].into();
	let leaf_hash_8: H256 = [8; 32].into();
	let leaf_hash_9: H256 = [9; 32].into();
	let node_0 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_0,
		leaf_hash_1,
	);
	let node_2 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_4,
		leaf_hash_5,
	);
	let node_3 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_6,
		leaf_hash_7,
	);
	let node_4 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		leaf_hash_8,
		leaf_hash_9,
	);
	let node_01 = proofs::hashing::sort_hash_of::<pallet_crowdloan_claim::ProofVerifier<Runtime>>(
		node_2, node_3,
	);

	sorted_hashed.push(leaf_hash_3);
	sorted_hashed.push(node_0);
	sorted_hashed.push(node_01);
	sorted_hashed.push(node_4);

	Contributor {
        proof: proofs::Proof::new(leaf_hash, sorted_hashed),
        signature: MultiSignature::Sr25519(sp_core::sr25519::Signature(
            hex::decode("a0db0cf026ffe5f0bc859681c9a1816e8a15991947753d6d7ecd1ac69c6e204c4ecf8f534d52ec9e76505e770dfa2b5d9614eca5e4d1de556dfa0de40dc7328f").unwrap().try_into().unwrap()
        )),
        parachain_account: 1,
        relaychain_account: contributor,
        contribution: amount,

    }
}

fn get_false_signature() -> MultiSignature {
	MultiSignature::Sr25519(sp_core::sr25519::Signature(
        hex::decode("111111111111111111111111111111111111111111111111111111111111111111112a8e699c7b6893f649edc630bfe11c7d278fb11b5f1084972669e250cc8c").unwrap().try_into().unwrap()
    ))
}

fn get_false_proof() -> proofs::Proof<H256> {
	// 10-leaf tree
	let mut sorted_hashed: Vec<H256> = Vec::new();

	sorted_hashed.push([0; 32].into());
	sorted_hashed.push([1; 32].into());
	sorted_hashed.push([2; 32].into());
	sorted_hashed.push([3; 32].into());
	sorted_hashed.push([4; 32].into());
	sorted_hashed.push([5; 32].into());
	sorted_hashed.push([6; 32].into());
	sorted_hashed.push([7; 32].into());
	sorted_hashed.push([8; 32].into());
	sorted_hashed.push([9; 32].into());

	proofs::Proof::new([10; 32].into(), sorted_hashed)
}

fn init_module() {
	CrowdloanClaim::initialize(RuntimeOrigin::signed(1), get_root(), 100, 0, 0, 400).unwrap();
	pallet_crowdloan_reward::Pallet::<Runtime>::initialize(
		RuntimeOrigin::signed(1),
		Perbill::from_percent(20),
		500,
		100,
	)
	.unwrap();
}

// ----------------------------------------------------------------------------
// Test cases
// ----------------------------------------------------------------------------
#[test]
fn test_valid_initialize_transaction() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			assert!(CrowdloanClaim::contributions().is_some());
			assert!(CrowdloanClaim::crowdloan_trie_index().is_some());
		})
}

#[test]
fn test_init_double() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			assert_noop!(
				CrowdloanClaim::initialize(RuntimeOrigin::signed(1), get_root(), 100, 0, 200, 400),
				CrowdloanClaimError::<Runtime>::PalletAlreadyInitialized
			);
		})
}

#[test]
fn test_init_non_admin() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			assert_noop!(
				CrowdloanClaim::initialize(RuntimeOrigin::signed(2), get_root(), 100, 0, 200, 400),
				CrowdloanClaimError::<Runtime>::MustBeAdministrator
			);
		})
}

#[test]
fn test_set_contribution_root() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			assert_ok!(CrowdloanClaim::set_lease_start(
				RuntimeOrigin::signed(1),
				999
			));
			assert_eq!(CrowdloanClaim::lease_start(), 999);
		})
}

#[test]
fn test_set_locked_at() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			assert_ok!(CrowdloanClaim::set_locked_at(RuntimeOrigin::signed(1), 999));
			assert_eq!(CrowdloanClaim::locked_at(), Some(999));
		})
}

#[test]
fn test_set_contributions_trie_index() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			let root = H256::zero();
			assert_ok!(CrowdloanClaim::set_contributions_root(
				RuntimeOrigin::signed(1),
				root
			));
			assert_eq!(CrowdloanClaim::contributions(), Some(root));
		})
}

#[test]
fn test_set_lease_start() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			assert_ok!(CrowdloanClaim::set_lease_start(
				RuntimeOrigin::signed(1),
				999
			));
			assert_eq!(CrowdloanClaim::lease_start(), 999);
		})
}

#[test]
fn test_set_lease_period() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			assert_ok!(CrowdloanClaim::set_lease_period(
				RuntimeOrigin::signed(1),
				999
			));
			assert_eq!(CrowdloanClaim::lease_period(), 999);
		})
}

#[test]
fn test_invalid_signed_claim_transaction() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			let alice = get_contributor();

			assert_noop!(
				CrowdloanClaim::claim_reward(
					RuntimeOrigin::signed(0),
					alice.relaychain_account,
					alice.parachain_account,
					get_false_signature(),
					alice.proof,
					alice.contribution
				),
				CrowdloanClaimError::<Runtime>::InvalidContributorSignature
			);
		})
}

#[test]
fn test_valid_claim() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			let bob = get_contributor();
			let bob_balance = Balances::free_balance(&bob.parachain_account);

			assert_ok!(CrowdloanClaim::claim_reward(
				RuntimeOrigin::signed(0),
				bob.relaychain_account.clone(),
				bob.parachain_account,
				bob.signature,
				bob.proof,
				bob.contribution
			));
			assert!(ProcessedClaims::<Runtime>::contains_key((
				&bob.relaychain_account,
				1
			)));

			assert_eq!(
				Vesting::vesting_balance(&bob.parachain_account),
				Some(3200000000000000)
			);
			assert_eq!(
				Balances::usable_balance(&bob.parachain_account),
				bob_balance + 800000000000000
			);
		});
}

#[test]
fn test_valid_claim_ext_signature() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {
			CrowdloanClaim::initialize(
				RuntimeOrigin::signed(1),
				get_root_for_ext_sig(),
				100,
				0,
				0,
				400,
			)
			.unwrap();
			pallet_crowdloan_reward::Pallet::<Runtime>::initialize(
				RuntimeOrigin::signed(1),
				Perbill::from_percent(20),
				500,
				100,
			)
			.unwrap();
		}))
		.execute_with(|| {
			let bob = get_contributor_ext_sig();
			let bob_balance = Balances::free_balance(&bob.parachain_account);

			assert_ok!(CrowdloanClaim::claim_reward(
				RuntimeOrigin::signed(0),
				bob.relaychain_account.clone(),
				bob.parachain_account,
				bob.signature,
				bob.proof,
				bob.contribution
			));
			assert!(ProcessedClaims::<Runtime>::contains_key((
				&bob.relaychain_account,
				1
			)));

			assert_eq!(
				Vesting::vesting_balance(&bob.parachain_account),
				Some(3200000000000000)
			);
			assert_eq!(
				Balances::usable_balance(&bob.parachain_account),
				bob_balance + 800000000000000
			);
		});
}

#[test]
fn test_valid_claim_but_lease_elapsed() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			System::set_block_number(601);

			let bob = get_contributor();
			assert_noop!(
				CrowdloanClaim::claim_reward(
					RuntimeOrigin::signed(0),
					bob.relaychain_account.clone(),
					bob.parachain_account,
					bob.signature,
					bob.proof,
					bob.contribution
				),
				Error::<Runtime>::OutOfLeasePeriod
			);
		});
}

#[test]
fn test_valid_claim_claimed_twice() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			let bob = get_contributor();
			assert_ok!(CrowdloanClaim::claim_reward(
				RuntimeOrigin::signed(0),
				bob.relaychain_account.clone(),
				bob.parachain_account,
				bob.signature,
				bob.proof,
				bob.contribution
			));
			assert!(ProcessedClaims::<Runtime>::contains_key((
				&bob.relaychain_account,
				1
			)));

			let bob = get_contributor();
			assert_noop!(
				CrowdloanClaim::claim_reward(
					RuntimeOrigin::signed(0),
					bob.relaychain_account.clone(),
					bob.parachain_account,
					bob.signature,
					bob.proof,
					bob.contribution
				),
				CrowdloanClaimError::<Runtime>::ClaimAlreadyProcessed
			);
		})
}

#[test]
fn test_invalid_claim_invalid_proof() {
	TestExternalitiesBuilder::default()
		.build(Some(init_module))
		.execute_with(|| {
			let alice = get_contributor();

			assert_noop!(
				Pallet::<Runtime>::claim_reward(
					RuntimeOrigin::signed(0),
					alice.relaychain_account,
					alice.parachain_account,
					alice.signature,
					get_false_proof(),
					alice.contribution
				),
				CrowdloanClaimError::<Runtime>::InvalidProofOfContribution
			);
		})
}

#[test]
fn test_invalid_claim_mod_not_initialized() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let alice = get_contributor();

			assert_noop!(
				CrowdloanClaim::claim_reward(
					RuntimeOrigin::signed(0),
					alice.relaychain_account,
					alice.parachain_account,
					alice.signature,
					alice.proof,
					alice.contribution
				),
				CrowdloanClaimError::<Runtime>::PalletNotInitialized
			);
		})
}

#[test]
fn test_claim_reward_requires_origin() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let alice = get_contributor();

			assert_noop!(
				CrowdloanClaim::claim_reward(
					RuntimeOrigin::none(),
					alice.relaychain_account,
					alice.parachain_account,
					alice.signature,
					alice.proof,
					alice.contribution
				),
				sp_runtime::DispatchError::BadOrigin
			);
		})
}
