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

use frame_benchmarking::{account, impl_benchmark_test_suite, v2::*};
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use parity_scale_codec::EncodeLike;
use sp_core::H256;

use super::*;

#[benchmarks(
	where
		T: Config<Balance = u128, Hash = H256, AnchorIdNonce = u128>,
		T::AccountId: EncodeLike<<T as frame_system::Config>::AccountId>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn set_anchor() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = account("acc_0", 0, 0);

		let document_id = 123;
		let document_version = 456;
		let hash = H256::from_low_u64_be(1);

		let _ = T::Currency::deposit_creating(
			&caller.clone().into(),
			T::Currency::minimum_balance() + T::DefaultAnchorDeposit::get(),
		);

		#[extrinsic_call]
		set_anchor(
			RawOrigin::Signed(caller),
			document_id,
			document_version,
			hash,
		);

		Ok(())
	}

	#[benchmark]
	fn remove_anchor() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = account("acc_0", 0, 0);

		let document_id = 123;
		let document_version = 456;
		let hash = H256::from_low_u64_be(1);
		let deposit = AnchorDeposit::<T>::get();
		let anchor_id = 1;

		let anchor = Anchor::<T> {
			anchor_id,
			account_id: caller.clone(),
			document_id,
			document_version,
			hash,
			deposit,
		};

		Anchors::<T>::insert(anchor_id, anchor.clone());
		DocumentAnchors::<T>::insert((document_id, document_version), anchor_id);
		PersonalAnchors::<T>::insert(caller.clone(), (document_id, document_version), anchor_id);

		#[extrinsic_call]
		remove_anchor(RawOrigin::Signed(caller), document_id, document_version);

		Ok(())
	}

	#[benchmark]
	fn set_deposit_value() -> Result<(), BenchmarkError> {
		let deposit = 2 * T::DefaultAnchorDeposit::get();

		#[extrinsic_call]
		set_deposit_value(RawOrigin::Root, deposit);

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}
