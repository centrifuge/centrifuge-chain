#![cfg(feature = "runtime-benchmarks")]
use frame_benchmarking::{account, benchmarks};
use frame_support::{StorageHasher, Twox128};
use frame_system::RawOrigin;
use sp_runtime::Perbill;

use super::*;

const CONTRIBUTION: u128 = 40000000000000000000;

benchmarks! {
  where_clause {where T: pallet_balances::Config}

  claim_reward_ed25519 {
		let caller: T::AccountId = account("claimer", 0, 0);
		let relay_account: T::RelayChainAccountId = get_account_relay_ed25519::<T>();
		init_pallets::<T>(relay_account.clone());
		let para_account: ParachainAccountIdOf<T> = get_account_para_ed25519::<T>();
		let identity_proof: sp_runtime::MultiSignature = get_signature_ed25519::<T>();
		let contribution: T::Balance = get_contribution::<T>(CONTRIBUTION);
		let contribution_proof: proofs::Proof<T::Hash> = get_proof::<T>(
			relay_account.clone(),
			contribution
		);

  }: claim_reward(RawOrigin::Signed(caller), relay_account, para_account, identity_proof, contribution_proof, contribution)
  verify {
		// TODO: Not sure if it is even possible to use the balances pallet here. But "T" does not implement the pallet_balances::Config
		//       so currently, I am not able to see a solution to get to the balances. Although, one might use storage directy. But I
		//       am lazy right now. The tests cover this quite well...
  }

	claim_reward_sr25519 {
		let caller: T::AccountId = account("claimer", 0, 0);
		let relay_account: T::RelayChainAccountId = get_account_relay_sr25519::<T>();
		init_pallets::<T>(relay_account.clone());
		let para_account: ParachainAccountIdOf<T> = get_account_para_sr25519::<T>();
		let identity_proof: sp_runtime::MultiSignature = get_signature_sr25519::<T>();
		let contribution: T::Balance = get_contribution::<T>(CONTRIBUTION);
		let contribution_proof: proofs::Proof<T::Hash> = get_proof::<T>(
			relay_account.clone(),
			contribution
		);
	}: claim_reward(RawOrigin::Signed(caller), relay_account, para_account, identity_proof, contribution_proof, contribution)
	verify{
		// TODO: Not sure if it is even possible to use the balances pallet here. But "T" does not implement the pallet_balances::Config
		//       so currently, I am not able to see a solution to get to the balances. Although, one might use storage directy. But I
		//       am lazy right now. The tests cover this quite well..
	}

	claim_reward_ecdsa {
		let caller: T::AccountId = account("claimer", 0, 0);
		let relay_account: T::RelayChainAccountId = get_account_relay_ecdsa::<T>();
		init_pallets::<T>(relay_account.clone());
		let para_account: ParachainAccountIdOf<T> = get_account_para_ecdsa::<T>();
		let identity_proof: sp_runtime::MultiSignature = get_signature_ecdsa::<T>();
		let contribution: T::Balance = get_contribution::<T>(CONTRIBUTION);
		let contribution_proof: proofs::Proof<T::Hash> = get_proof::<T>(
			relay_account.clone(),
			contribution
		);

	  }: claim_reward(RawOrigin::Signed(caller), relay_account, para_account, identity_proof, contribution_proof, contribution)
	  verify {
		// TODO: Not sure if it is even possible to use the balances pallet here. But "T" does not implement the pallet_balances::Config
		//       so currently, I am not able to see a solution to get to the balances. Although, one might use storage directy. But I
		//       am lazy right now. The tests cover this quite well...
	  }

  initialize {
		let contributions: RootHashOf<T> = get_root::<T>(
			get_account_relay_sr25519::<T>(),
			get_contribution::<T>(CONTRIBUTION)
		);
		let locked_at: BlockNumberFor<T> = 1u32.into();
		let index: TrieIndex = 1u32.into();
		let lease_start: BlockNumberFor<T> = 1u32.into();
		let lease_period: BlockNumberFor<T> = 1u32.into();
  }: _(RawOrigin::Root, contributions, locked_at, index, lease_start, lease_period)
  verify {
		assert!(Pallet::<T>::contributions().is_some());
		assert!(Pallet::<T>::locked_at().is_some());
		assert!(Pallet::<T>::crowdloan_trie_index().is_some());
		assert_eq!(Pallet::<T>::lease_start(), 1u32.into());
		assert_eq!(Pallet::<T>::lease_period(), 1u32.into());
  }

  set_lease_start{
	let start: BlockNumberFor<T> = 1u32.into();
  }: _(RawOrigin::Root, start)
  verify {
		assert_eq!(Pallet::<T>::lease_start(), 1u32.into());
  }

  set_lease_period{
	let period: BlockNumberFor<T> = 1u32.into();
  }: _(RawOrigin::Root, period)
  verify {
		assert_eq!(Pallet::<T>::lease_period(), 1u32.into());
  }

  set_contributions_root {
	let root: RootHashOf<T> = get_root::<T>(
			get_account_relay_sr25519::<T>(),
			get_contribution::<T>(CONTRIBUTION)
	);
  }: _(RawOrigin::Root, root)
  verify {
		assert!(Pallet::<T>::contributions().is_some());
  }

  set_locked_at {
	  let locked: BlockNumberFor<T> = 1u32.into();
  }: _(RawOrigin::Root, locked)
  verify {
		assert!(Pallet::<T>::locked_at().is_some());
  }

  set_crowdloan_trie_index {
	  let index: TrieIndex = 1u32.into();
  }: _(RawOrigin::Root, index)
  verify {
		assert!(Pallet::<T>::crowdloan_trie_index().is_some());
  }
}

// Helper functions from here on
//
fn get_contribution<T: Config>(amount: u128) -> T::Balance {
	match amount.try_into() {
		Ok(contribution) => contribution,
		Err(_) => panic!(),
	}
}

#[allow(dead_code)]
fn get_balance<T: Config>(amount: u128) -> T::Balance {
	match amount.try_into() {
		Ok(contribution) => contribution,
		Err(_) => panic!(),
	}
}

// In order to detangle from sp-core/fullCrypto which seems to be missing some
// trait implementations
#[derive(parity_scale_codec::Encode, parity_scale_codec::Decode)]
struct Signature(pub [u8; 64]);

#[derive(parity_scale_codec::Encode, parity_scale_codec::Decode)]
struct SignatureEcdsa(pub [u8; 65]);

#[derive(parity_scale_codec::Encode, parity_scale_codec::Decode)]
enum MultiSignature {
	/// An Ed25519 signature.
	Ed25519(Signature),
	/// An Sr25519 signature.
	Sr25519(Signature),
	/// An ECDSA/SECP256k1 signature.
	Ecdsa(SignatureEcdsa),
}

// All accounts in the following are derived from this Mnemonic
//
// "flight client wild replace umbrella april addict below deer inch mix
// surface"
//

fn get_account_para_ed25519<T: Config>() -> ParachainAccountIdOf<T> {
	let pub_key: [u8; 32] = [
		130, 168, 6, 216, 161, 211, 10, 240, 194, 245, 185, 187, 131, 189, 246, 132, 115, 145, 87,
		11, 164, 80, 205, 180, 87, 88, 208, 16, 60, 59, 83, 186,
	];

	parity_scale_codec::Decode::decode(&mut &pub_key[..]).unwrap()
}

fn get_account_para_ecdsa<T: Config>() -> ParachainAccountIdOf<T> {
	let pub_key: [u8; 32] = [
		89, 211, 18, 12, 18, 109, 171, 175, 21, 236, 203, 33, 33, 168, 153, 55, 198, 227, 184, 139,
		77, 115, 132, 73, 59, 235, 90, 175, 221, 88, 44, 247,
	];

	parity_scale_codec::Decode::decode(&mut &pub_key[..]).unwrap()
}

fn get_account_para_sr25519<T: Config>() -> ParachainAccountIdOf<T> {
	let pub_key: [u8; 32] = [
		202, 13, 159, 82, 100, 222, 166, 237, 52, 113, 173, 161, 100, 206, 112, 67, 188, 178, 135,
		53, 61, 178, 143, 121, 157, 182, 189, 207, 59, 166, 7, 92,
	];

	parity_scale_codec::Decode::decode(&mut &pub_key[..]).unwrap()
}

fn get_signature_ecdsa<T: Config>() -> sp_runtime::MultiSignature {
	let msg: [u8; 65] = [
		234, 70, 108, 203, 158, 59, 224, 51, 248, 194, 209, 45, 0, 146, 83, 185, 172, 19, 254, 12,
		148, 232, 249, 183, 131, 64, 115, 3, 39, 230, 101, 120, 87, 230, 202, 183, 162, 167, 122,
		95, 186, 231, 179, 183, 119, 241, 166, 55, 10, 21, 243, 228, 147, 73, 2, 84, 34, 211, 51,
		40, 245, 198, 16, 140, 0,
	];

	let local_sig = SignatureEcdsa(msg);
	let local_multisig = MultiSignature::Ecdsa(local_sig);

	parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&local_multisig).as_slice(),
	)
	.unwrap()
}

fn get_signature_sr25519<T: Config>() -> sp_runtime::MultiSignature {
	let msg: [u8; 64] = [
		132, 172, 248, 32, 17, 107, 155, 94, 246, 87, 44, 158, 2, 230, 220, 225, 170, 217, 104,
		189, 211, 57, 98, 161, 179, 160, 79, 23, 185, 165, 250, 1, 160, 253, 160, 116, 27, 168, 19,
		82, 30, 175, 146, 222, 178, 143, 46, 84, 15, 162, 146, 212, 244, 39, 166, 198, 137, 116,
		30, 14, 184, 17, 212, 141,
	];

	let local_sig = Signature(msg);
	let local_multisig = MultiSignature::Sr25519(local_sig);

	parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&local_multisig).as_slice(),
	)
	.unwrap()
}

fn get_signature_ed25519<T: Config>() -> sp_runtime::MultiSignature {
	let msg: [u8; 64] = [
		138, 180, 126, 32, 200, 234, 37, 182, 93, 251, 36, 179, 98, 233, 42, 246, 118, 207, 203,
		108, 89, 229, 1, 218, 194, 32, 206, 88, 199, 27, 224, 54, 90, 214, 233, 122, 229, 50, 175,
		248, 142, 175, 37, 185, 212, 199, 93, 92, 58, 91, 94, 29, 55, 42, 67, 107, 119, 155, 143,
		192, 66, 181, 236, 8,
	];
	let local_sig = Signature(msg);
	let local_multisig = MultiSignature::Ed25519(local_sig);

	parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&local_multisig).as_slice(),
	)
	.unwrap()
}

fn get_account_relay_ecdsa<T: Config>() -> T::RelayChainAccountId {
	let pub_key: [u8; 32] = [
		89, 211, 18, 12, 18, 109, 171, 175, 21, 236, 203, 33, 33, 168, 153, 55, 198, 227, 184, 139,
		77, 115, 132, 73, 59, 235, 90, 175, 221, 88, 44, 247,
	];

	parity_scale_codec::Decode::decode(&mut &pub_key[..]).unwrap()
}

fn get_account_relay_sr25519<T: Config>() -> T::RelayChainAccountId {
	let pub_key: [u8; 32] = [
		202, 13, 159, 82, 100, 222, 166, 237, 52, 113, 173, 161, 100, 206, 112, 67, 188, 178, 135,
		53, 61, 178, 143, 121, 157, 182, 189, 207, 59, 166, 7, 92,
	];

	parity_scale_codec::Decode::decode(&mut &pub_key[..]).unwrap()
}

fn get_account_relay_ed25519<T: Config>() -> T::RelayChainAccountId {
	let pub_key: [u8; 32] = [
		130, 168, 6, 216, 161, 211, 10, 240, 194, 245, 185, 187, 131, 189, 246, 132, 115, 145, 87,
		11, 164, 80, 205, 180, 87, 88, 208, 16, 60, 59, 83, 186,
	];

	parity_scale_codec::Decode::decode(&mut &pub_key[..]).unwrap()
}

fn get_proof<T: Config>(
	relay: T::RelayChainAccountId,
	contribution: T::Balance,
) -> proofs::Proof<T::Hash> {
	let mut v: Vec<u8> = relay.encode();
	v.extend(contribution.encode());
	let leaf_hash: T::Hash = <T as frame_system::Config>::Hashing::hash(&v);

	let mut sorted_hashed: Vec<T::Hash> = Vec::new();

	// 10-leaf tree
	let leaf_hash_0: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[0u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_1: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[1u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_3: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[2u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_4: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[3u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_5: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[4u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_6: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[5u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_7: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[6u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_8: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[7u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_9: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[8u32; 32]).as_slice(),
	)
	.unwrap();
	let node_0 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(leaf_hash_0, leaf_hash_1);
	let node_2 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(leaf_hash_4, leaf_hash_5);
	let node_3 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(leaf_hash_6, leaf_hash_7);
	let node_4 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(leaf_hash_8, leaf_hash_9);
	let node_01 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(node_2, node_3);

	sorted_hashed.push(leaf_hash_3);
	sorted_hashed.push(node_0);
	sorted_hashed.push(node_01);
	sorted_hashed.push(node_4);

	proofs::Proof::new(leaf_hash, sorted_hashed)
}

fn get_root<T: Config>(relay: T::RelayChainAccountId, contribution: T::Balance) -> RootHashOf<T> {
	let mut v: Vec<u8> = relay.encode();
	v.extend(contribution.encode());
	let leaf_hash: T::Hash = <T as frame_system::Config>::Hashing::hash(&v);

	// 10-leaf tree
	let leaf_hash_0: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[0u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_1: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[1u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_2: T::Hash = leaf_hash;
	let leaf_hash_3: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[2u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_4: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[3u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_5: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[4u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_6: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[5u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_7: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[6u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_8: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[7u32; 32]).as_slice(),
	)
	.unwrap();
	let leaf_hash_9: T::Hash = parity_scale_codec::Decode::decode(
		&mut parity_scale_codec::Encode::encode(&[8u32; 32]).as_slice(),
	)
	.unwrap();
	let node_0 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(leaf_hash_0, leaf_hash_1);
	let node_1 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(leaf_hash_2, leaf_hash_3);
	let node_2 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(leaf_hash_4, leaf_hash_5);
	let node_3 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(leaf_hash_6, leaf_hash_7);
	let node_4 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(leaf_hash_8, leaf_hash_9);
	let node_00 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(node_0, node_1);
	let node_01 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(node_2, node_3);
	let node_000 = proofs::hashing::sort_hash_of::<ProofVerifier<T>>(node_00, node_01);

	proofs::hashing::sort_hash_of::<ProofVerifier<T>>(node_000, node_4).into()
}

fn init_pallets<T: Config>(relay_account: T::RelayChainAccountId) {
	// Inject storage here. Using the
	<Contributions<T>>::put(get_root::<T>(
		relay_account,
		get_contribution::<T>(CONTRIBUTION),
	));
	<CrowdloanTrieIndex<T>>::put(Into::<TrieIndex>::into(100u32));
	<LockedAt<T>>::put(Into::<BlockNumberFor<T>>::into(0u32));
	<LeaseStart<T>>::put(Into::<BlockNumberFor<T>>::into(0u32));
	<LeasePeriod<T>>::put(Into::<BlockNumberFor<T>>::into(400u32));
	<CurrIndex<T>>::put(Into::<Index>::into(1u32));

	let vesting_start_key = create_final_key_crowdloan_reward(b"VestingStart");
	let vesting_start: BlockNumberFor<T> = 100u32.into();
	frame_support::storage::unhashed::put(&vesting_start_key, &vesting_start);

	let vesting_period_key = create_final_key_crowdloan_reward(b"VestingPeriod");
	let vesting_period: BlockNumberFor<T> = 500u32.into();
	frame_support::storage::unhashed::put(&vesting_period_key, &vesting_period);

	let direct_payout_ratio_key = create_final_key_crowdloan_reward(b"DirectPayoutRatio");
	let direct_payout_ratio: Perbill = Perbill::from_percent(20u32);
	frame_support::storage::unhashed::put(&direct_payout_ratio_key, &direct_payout_ratio);
}

fn create_final_key_crowdloan_reward(element: &[u8]) -> [u8; 32] {
	let mut final_key = [0u8; 32];
	final_key[0..16].copy_from_slice(&Twox128::hash(b"CrowdloanReward"));
	final_key[16..32].copy_from_slice(&Twox128::hash(element));
	final_key
}
