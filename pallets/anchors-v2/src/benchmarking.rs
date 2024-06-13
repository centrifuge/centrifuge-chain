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

use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use parity_scale_codec::EncodeLike;
use scale_info::prelude::format;
use sp_core::H256;
use sp_runtime::traits::Hash;

use super::*;

benchmarks! {
	where_clause {
	where
		T: Config<Balance = u128, DocumentId = u128, DocumentVersion = u64, Hash = H256>,
		T::AccountId: EncodeLike<<T as frame_system::Config>::AccountId>,
	}

	set_anchor {
		let caller: T::AccountId = account("acc_0", 0, 0);

		let document_id = 123;
		let document_version = 456;
		let hash = H256::random();
		let deposit = AnchorDeposit::<T>::get();

		let anchor = AnchorOf::<T> {
			account_id: caller.clone(),
			document_id,
			document_version,
			hash,
			deposit,
		};

		let _ = T::Currency::deposit_creating(&caller.clone().into(), T::Currency::minimum_balance() + T::DefaultAnchorDeposit::get());
		let origin = RawOrigin::Signed(caller.clone());
	}: set_anchor(origin.clone(), document_id, document_version, hash)
	verify {
		assert_eq!(Anchors::<T>::get((document_id, document_version), caller.clone()), Some(anchor.clone()));
		assert_eq!(PersonalAnchors::<T>::get((caller, document_id, document_version)), Some(anchor));
	}

	remove_anchor {
		let caller: T::AccountId = account("acc_0", 0, 0);

		let document_id = 123;
		let document_version = 456;
		let hash = H256::random();
		let deposit = AnchorDeposit::<T>::get();

		let anchor = AnchorOf::<T> {
			account_id: caller.clone(),
			document_id,
			document_version,
			hash,
			deposit,
		};

		Anchors::<T>::insert((document_id, document_version), caller.clone(), anchor.clone());
		PersonalAnchors::<T>::insert((caller.clone(), document_id, document_version), anchor);

		let origin = RawOrigin::Signed(caller.clone());
	}: remove_anchor(origin.clone(), document_id, document_version)
	verify {
		assert_eq!(Anchors::<T>::get((document_id, document_version), caller.clone()), None);
		assert_eq!(PersonalAnchors::<T>::get((caller, document_id, document_version)), None);
	}

	set_deposit {
		let deposit = 2 * T::DefaultAnchorDeposit::get();
	}: set_deposit(RawOrigin::Root, deposit)
	verify {
		assert_eq!(AnchorDeposit::<T>::get(), 2 * T::DefaultAnchorDeposit::get());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);
