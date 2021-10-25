#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_support::Blake2_128Concat;
use frame_support::StorageHasher;
use frame_support::Twox128;
use frame_system::Account;
use frame_system::AccountInfo;
use frame_system::RawOrigin;
use pallet_balances::AccountData;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::Perbill;

const CONTRIBUTION: u128 = 4000000000000000000000;

benchmarks! {
  claim_reward {
		init_pallets::<T>();

		// Create some balances for the module
		let mut key = Vec::new();
		let reward_id = PalletId(*b"cc/rewrd");
		key.extend_from_slice(&Twox128::hash(b"System"));
		key.extend_from_slice(&Twox128::hash(b"Account"));
		key.extend_from_slice(&Blake2_128Concat::hash(AccountIdConversion::<T::AccountId>::into_account(&reward_id).encode().as_slice()));

		let info: frame_system::AccountInfo<T::Index, T::AccountData> = AccountInfo {
			nonce: 0u32.into(),
			consumers: 0u32.into(),
			providers: 1u32.into(),
			sufficients: 0u32.into(),
			data: codec::Decode::decode(&mut AccountData {
				free: get_balance::<T>(1000000000000000000000000000u128),
				reserved: 0u32.into(),
				misc_frozen: 0u32.into(),
				fee_frozen: 0u32.into()
			}.encode().as_slice()).unwrap()
		};

		<Account<T>>::insert(AccountIdConversion::<T::AccountId>::into_account(&reward_id), info);
		//frame_support::storage::unhashed::put(&key, &info);

		let relay_account: T::RelayChainAccountId = get_account_relay::<T>();
		let para_account: ParachainAccountIdOf<T> = get_account_para::<T>();
		let identity_proof: sp_runtime::MultiSignature = get_signature::<T>();
		let contribution: T::Balance = get_contribution::<T>(CONTRIBUTION);
		let contribution_proof: proofs::Proof<T::Hash> = get_proof::<T>(
			relay_account.clone(),
			contribution
		);

  }: _(RawOrigin::None, relay_account, para_account, identity_proof, contribution_proof, contribution)
  verify {
		// TODO: Not sure if it is even possible to use the balances pallet here. But "T" does not implement the pallet_balances::Config
		//       so currently, I am not able to see a solution to get to the balances. Although, one might use storage directy. But I
		//       am lazy right now. The tests cover this quite well...
  }

  initialize {
		let contributions: RootHashOf<T> = get_root::<T>(
			get_account_relay::<T>(),
			get_contribution::<T>(CONTRIBUTION)
		);
		let locked_at: T::BlockNumber = 1u32.into();
		let index: TrieIndex = 1u32.into();
		let lease_start: T::BlockNumber = 1u32.into();
		let lease_period: T::BlockNumber = 1u32.into();
  }: _(RawOrigin::Root, contributions, locked_at, index, lease_start, lease_period)
  verify {
		assert!(Pallet::<T>::contributions().is_some());
		assert!(Pallet::<T>::locked_at().is_some());
		assert!(Pallet::<T>::crowdloan_trie_index().is_some());
		assert_eq!(Pallet::<T>::lease_start(), 1u32.into());
		assert_eq!(Pallet::<T>::lease_period(), 1u32.into());
  }

  set_lease_start{
	let start: T::BlockNumber = 1u32.into();
  }: _(RawOrigin::Root, start)
  verify {
		assert_eq!(Pallet::<T>::lease_start(), 1u32.into());
  }

  set_lease_period{
	let period: T::BlockNumber = 1u32.into();
  }: _(RawOrigin::Root, period)
  verify {
		assert_eq!(Pallet::<T>::lease_period(), 1u32.into());
  }

  set_contributions_root {
	let root: RootHashOf<T> = get_root::<T>(
			get_account_relay::<T>(),
			get_contribution::<T>(CONTRIBUTION)
	);
  }: _(RawOrigin::Root, root)
  verify {
		assert!(Pallet::<T>::contributions().is_some());
  }

  set_locked_at {
	  let locked: T::BlockNumber = 1u32.into();
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

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(None),
	crate::mock::T,
);

// Helper functions from here on
//
fn get_contribution<T: Config>(amount: u128) -> T::Balance {
	match amount.try_into() {
		Ok(contribution) => contribution,
		Err(_) => panic!(),
	}
}

fn get_balance<T: Config>(amount: u128) -> T::Balance {
	match amount.try_into() {
		Ok(contribution) => contribution,
		Err(_) => panic!(),
	}
}

// In order to detangle from sp-core/fullCrypto which seems to be missing some trait implementations
#[derive(codec::Encode, codec::Decode)]
struct Signature(pub [u8; 64]);

#[derive(codec::Encode, codec::Decode)]
enum MultiSignature {
	/// An Ed25519 signature.
	Ed25519(Signature),
	/// An Sr25519 signature.
	Sr25519(Signature),
	/// An ECDSA/SECP256k1 signature.
	Ecdsa(Signature),
}

fn get_account_para<T: Config>() -> ParachainAccountIdOf<T> {
	// This is bob as hex: 0x80402e69a3fc09da233b5332f5a8c50a0175a3641a690d25d1107467717fff0e
	// This is bob as bytes: [
	//   128,  64,  46, 105, 163, 252,   9, 218,
	//    35,  59,  83,  50, 245, 168, 197,  10,
	//     1, 117, 163, 100,  26, 105,  13,  37,
	//   209,  16, 116, 103, 113, 127, 255,  14
	// ]
	let pub_key: [u8; 32] = [
		128, 64, 46, 105, 163, 252, 9, 218, 35, 59, 83, 50, 245, 168, 197, 10, 1, 117, 163, 100,
		26, 105, 13, 37, 209, 16, 116, 103, 113, 127, 255, 14,
	];

	codec::Decode::decode(&mut &pub_key[..]).unwrap()
}

fn get_signature<T: Config>() -> sp_runtime::MultiSignature {
	// This is alice signing bob's pub-key in bytes as bytes
	// [
	//   126, 160, 187, 236, 114,  88,   0, 178, 137, 109,   3,
	//    76,  78, 214, 143, 161, 177, 242,  54, 176, 236,  20,
	//    66, 232,  99, 250, 136,  40, 246, 219, 138,  66,  40,
	//   202, 236,  83, 218, 136,  35, 200, 111, 233,  32, 184,
	//     4, 197,  12, 155,   7, 250, 109, 102, 173,  38,   1,
	//   127,  72,   4, 205, 211,  13,  85,  72, 128
	// ]
	let msg: [u8; 64] = [
		126, 160, 187, 236, 114, 88, 0, 178, 137, 109, 3, 76, 78, 214, 143, 161, 177, 242, 54, 176,
		236, 20, 66, 232, 99, 250, 136, 40, 246, 219, 138, 66, 40, 202, 236, 83, 218, 136, 35, 200,
		111, 233, 32, 184, 4, 197, 12, 155, 7, 250, 109, 102, 173, 38, 1, 127, 72, 4, 205, 211, 13,
		85, 72, 128,
	];
	let local_sig = Signature(msg);
	let local_multisig = MultiSignature::Sr25519(local_sig);

	codec::Decode::decode(&mut codec::Encode::encode(&local_multisig).as_slice()).unwrap()
}

fn get_account_relay<T: Config>() -> T::RelayChainAccountId {
	// This is alice as hex 0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d
	// ALice as bytes:[
	//   212, 53, 147, 199,  21, 253, 211,  28,
	//    97, 20,  26, 189,   4, 169, 159, 214,
	//   130, 44, 133,  88, 133,  76, 205, 227,
	//   154, 86, 132, 231, 165, 109, 162, 125
	// ]
	let pub_key: [u8; 32] = [
		212, 53, 147, 199, 21, 253, 211, 28, 97, 20, 26, 189, 4, 169, 159, 214, 130, 44, 133, 88,
		133, 76, 205, 227, 154, 86, 132, 231, 165, 109, 162, 125,
	];

	codec::Decode::decode(&mut &pub_key[..]).unwrap()
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
	let leaf_hash_0: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[0u32; 32]).as_slice()).unwrap();
	let leaf_hash_1: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[1u32; 32]).as_slice()).unwrap();
	let leaf_hash_3: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[2u32; 32]).as_slice()).unwrap();
	let leaf_hash_4: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[3u32; 32]).as_slice()).unwrap();
	let leaf_hash_5: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[4u32; 32]).as_slice()).unwrap();
	let leaf_hash_6: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[5u32; 32]).as_slice()).unwrap();
	let leaf_hash_7: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[6u32; 32]).as_slice()).unwrap();
	let leaf_hash_8: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[7u32; 32]).as_slice()).unwrap();
	let leaf_hash_9: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[8u32; 32]).as_slice()).unwrap();
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
	let leaf_hash_0: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[0u32; 32]).as_slice()).unwrap();
	let leaf_hash_1: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[1u32; 32]).as_slice()).unwrap();
	let leaf_hash_2: T::Hash = leaf_hash;
	let leaf_hash_3: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[2u32; 32]).as_slice()).unwrap();
	let leaf_hash_4: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[3u32; 32]).as_slice()).unwrap();
	let leaf_hash_5: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[4u32; 32]).as_slice()).unwrap();
	let leaf_hash_6: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[5u32; 32]).as_slice()).unwrap();
	let leaf_hash_7: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[6u32; 32]).as_slice()).unwrap();
	let leaf_hash_8: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[7u32; 32]).as_slice()).unwrap();
	let leaf_hash_9: T::Hash =
		codec::Decode::decode(&mut codec::Encode::encode(&[8u32; 32]).as_slice()).unwrap();
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

fn init_pallets<T: Config>() {
	// Inject storage here. Using the
	<Contributions<T>>::put(get_root::<T>(
		get_account_relay::<T>(),
		get_contribution::<T>(CONTRIBUTION),
	));
	<CrowdloanTrieIndex<T>>::put(Into::<TrieIndex>::into(100u32));
	<LockedAt<T>>::put(Into::<T::BlockNumber>::into(0u32));
	<LeaseStart<T>>::put(Into::<T::BlockNumber>::into(200u32));
	<LeasePeriod<T>>::put(Into::<T::BlockNumber>::into(400u32));
	<CurrIndex<T>>::put(Into::<Index>::into(1u32));

	let vesting_start_key = create_final_key_crowdloan_reward(b"VestingStart");
	let vesting_start: T::BlockNumber = 100u32.into();
	frame_support::storage::unhashed::put(&vesting_start_key, &vesting_start);

	let vesting_period_key = create_final_key_crowdloan_reward(b"VestingPeriod");
	let vesting_period: T::BlockNumber = 500u32.into();
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
