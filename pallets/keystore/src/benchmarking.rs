// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use codec::EncodeLike;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use scale_info::prelude::format;
use sp_runtime::traits::Hash;

use super::*;

benchmarks! {
	where_clause {
	where
		T: Config<Balance = u128>,
		T::AccountId: EncodeLike<<T as frame_system::Config>::AccountId>,
	}

	add_keys {
		let n in 1..T::MaxKeys::get();
		let caller: T::AccountId = account("acc_0", 0, 0);
		let test_keys: Vec<AddKey<T::Hash>> = build_test_keys::<T>(n);
		T::Currency::deposit_creating(&caller, T::DefaultKeyDeposit::get() * n as u128);
		let origin = RawOrigin::Signed(caller);
	}: add_keys(origin, test_keys)
	verify {
		assert_eq!(Keys::<T>::iter().collect::<Vec<_>>().len() as u32, n);
	}

	revoke_keys {
		let n in 1..T::MaxKeys::get();
		let caller: T::AccountId = account("acc_1", 1, 1);
		let test_keys: Vec<AddKey<T::Hash>> = build_test_keys::<T>(n);

		add_keys_to_storage::<T>(caller.clone(), test_keys.clone());

		let key_hashes: Vec<T::Hash> = test_keys.iter().map(|add_key| add_key.key).collect();
		let origin = RawOrigin::Signed(caller.clone());
	}: revoke_keys(origin, key_hashes, KeyPurpose::P2PDiscovery)
	verify {
		assert_eq!(Keys::<T>::iter().collect::<Vec<_>>().len() as u32, n);
		assert!(all_keys_are_revoked::<T>(caller));
	}

	set_deposit {
		let deposit = 2 * T::DefaultKeyDeposit::get();
	}: set_deposit(RawOrigin::Root, deposit)
	verify {
		assert_eq!(KeyDeposit::<T>::get(), 2 * T::DefaultKeyDeposit::get());
	}
}

fn all_keys_are_revoked<T: Config>(account_id: T::AccountId) -> bool {
	for (_, key) in Keys::<T>::iter_prefix(account_id) {
		if key.revoked_at.is_none() {
			return false;
		}
	}

	true
}

fn add_keys_to_storage<T: Config>(account_id: T::AccountId, keys: Vec<AddKey<T::Hash>>) {
	for key in keys.iter() {
		let key_id: KeyId<T::Hash> = (key.key, key.purpose.clone());

		Keys::<T>::insert(
			account_id.clone(),
			key_id,
			Key {
				purpose: key.purpose.clone(),
				key_type: key.key_type.clone(),
				revoked_at: None,
				deposit: T::DefaultKeyDeposit::get(),
			},
		);
	}
}

fn build_test_keys<T: Config>(n: u32) -> Vec<AddKey<T::Hash>> {
	let mut keys: Vec<AddKey<T::Hash>> = Vec::new();

	for i in 0..n {
		let hash = format!("some_hash_{i}");

		let key_hash = T::Hashing::hash(hash.as_bytes());

		keys.push(AddKey::<T::Hash> {
			key: key_hash,
			purpose: KeyPurpose::P2PDiscovery,
			key_type: KeyType::ECDSA,
		});
	}

	keys
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);
