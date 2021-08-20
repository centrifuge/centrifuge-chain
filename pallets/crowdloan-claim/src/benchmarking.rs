#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_support::Blake2_128Concat;
use frame_support::StorageHasher;
use frame_support::Twox128;
use frame_system::AccountInfo;
use frame_system::RawOrigin;
use pallet_balances::AccountData;
use sp_core::Pair;
use sp_io::hashing::blake2_256;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::Perbill;
use sp_std::vec;

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
				free: get_balance::<T>(9999999999999999u64),
				reserved: 0u32.into(),
				misc_frozen: 0u32.into(),
				fee_frozen: 0u32.into()
			}.encode().as_slice()).unwrap()
		};
		frame_support::storage::unhashed::put(&key, &info);

		let relay_account: T::RelayChainAccountId = get_account_relay::<T>("contributor", 1, 1);
		let para_account: ParachainAccountIdOf<T> = get_account_para::<T>("rewardy", 1, 1);
		let identity_proof: MultiSignature = MultiSignature::Sr25519(get_signature::<T>(
			("contributor", 1, 1),
			para_account.clone(),
		));
		let contribution: ContributionAmountOf<T> = get_contribution::<T>(400);
		let contribution_proof: Vec<T::Hash> = get_proof::<T>(
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
			get_account_relay::<T>("contributor", 1, 1),
			get_contribution::<T>(400)
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
			get_account_relay::<T>("contributor", 1, 1),
			get_contribution::<T>(400)
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
	crate::mock::MockRuntime,
);

// Helper functions from here on
//
fn get_contribution<T: Config>(amount: u64) -> ContributionAmountOf<T> {
	match amount.try_into() {
		Ok(contribution) => contribution,
		Err(_) => panic!(),
	}
}

fn get_balance<T: Config>(amount: u64) -> T::Balance {
	match amount.try_into() {
		Ok(contribution) => contribution,
		Err(_) => panic!(),
	}
}

fn get_account_para<T: Config>(
	name: &'static str,
	index: u32,
	seed: u32,
) -> ParachainAccountIdOf<T> {
	let entropy = (name, index, seed).using_encoded(blake2_256);
	let (keypair, _) = sp_core::sr25519::Pair::from_entropy(&entropy[..], None);

	codec::Decode::decode(&mut &keypair.public().0[..]).unwrap()
}

fn get_signature<T: Config>(
	relay: (&'static str, u32, u32),
	para: ParachainAccountIdOf<T>,
) -> sp_core::sr25519::Signature {
	let entropy = (relay.0, relay.1, relay.2).using_encoded(blake2_256);
	let (keypair, _) = sp_core::sr25519::Pair::from_entropy(&entropy[..], None);
	let msg = keypair.sign(para.encode().as_slice());

	sp_core::sr25519::Signature(msg.0)
}

fn get_account_relay<T: Config>(
	name: &'static str,
	index: u32,
	seed: u32,
) -> T::RelayChainAccountId {
	let entropy = (name, index, seed).using_encoded(blake2_256);
	let (keypair, _) = sp_core::sr25519::Pair::from_entropy(&entropy[..], None);

	codec::Decode::decode(&mut &keypair.public().0[..]).unwrap()
}

fn get_proof<T: Config>(
	relay: T::RelayChainAccountId,
	contribution: ContributionAmountOf<T>,
) -> Vec<T::Hash> {
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
	let node_0 = Pallet::<T>::sorted_hash_of(&leaf_hash_0, &leaf_hash_1);
	let node_1 = Pallet::<T>::sorted_hash_of(&leaf_hash_2, &leaf_hash_3);
	let node_2 = Pallet::<T>::sorted_hash_of(&leaf_hash_4, &leaf_hash_5);
	let node_3 = Pallet::<T>::sorted_hash_of(&leaf_hash_6, &leaf_hash_7);
	let node_4 = Pallet::<T>::sorted_hash_of(&leaf_hash_8, &leaf_hash_9);
	let node_00 = Pallet::<T>::sorted_hash_of(&node_0, &node_1);
	let node_01 = Pallet::<T>::sorted_hash_of(&node_2, &node_3);
	let node_000 = Pallet::<T>::sorted_hash_of(&node_00, &node_01);
	let _node_root = Pallet::<T>::sorted_hash_of(&node_000, &node_4);

	let proof: Vec<T::Hash> = vec![
		leaf_hash_3.into(),
		node_0.into(),
		node_01.into(),
		node_4.into(),
	];

	proof.into()
}

fn get_root<T: Config>(
	relay: T::RelayChainAccountId,
	contribution: ContributionAmountOf<T>,
) -> RootHashOf<T> {
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
	let node_0 = Pallet::<T>::sorted_hash_of(&leaf_hash_0, &leaf_hash_1);
	let node_1 = Pallet::<T>::sorted_hash_of(&leaf_hash_2, &leaf_hash_3);
	let node_2 = Pallet::<T>::sorted_hash_of(&leaf_hash_4, &leaf_hash_5);
	let node_3 = Pallet::<T>::sorted_hash_of(&leaf_hash_6, &leaf_hash_7);
	let node_4 = Pallet::<T>::sorted_hash_of(&leaf_hash_8, &leaf_hash_9);
	let node_00 = Pallet::<T>::sorted_hash_of(&node_0, &node_1);
	let node_01 = Pallet::<T>::sorted_hash_of(&node_2, &node_3);
	let node_000 = Pallet::<T>::sorted_hash_of(&node_00, &node_01);

	Pallet::<T>::sorted_hash_of(&node_000, &node_4).into()
}

fn init_pallets<T: Config>() {
	// Inject storage here. Using the
	<Contributions<T>>::put(get_root::<T>(
		get_account_relay::<T>("contributor", 1, 1),
		get_contribution::<T>(400),
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

	let conversion_rate_key = create_final_key_crowdloan_reward(b"ConversionRate");
	let conversion_rate: u64 = 100u64;
	frame_support::storage::unhashed::put(&conversion_rate_key, &conversion_rate);
}

fn create_final_key_crowdloan_reward(element: &[u8]) -> [u8; 32] {
	let mut final_key = [0u8; 32];
	final_key[0..16].copy_from_slice(&Twox128::hash(b"CrowdloanReward"));
	final_key[16..32].copy_from_slice(&Twox128::hash(element));
	final_key
}
