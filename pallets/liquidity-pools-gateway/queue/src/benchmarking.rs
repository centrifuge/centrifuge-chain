// Copyright 2024 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::liquidity_pools::test_util::Message as LPTestMessage;
use frame_benchmarking::{account, impl_benchmark_test_suite, v2::*};
use frame_system::RawOrigin;
use parity_scale_codec::EncodeLike;

use super::*;

#[benchmarks(
	where
		T: Config<Message = LPTestMessage>,
		T::AccountId: EncodeLike<<T as frame_system::Config>::AccountId>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn process_message() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = account("acc_0", 0, 0);
		let message = LPTestMessage {};
		let nonce = T::MessageNonce::one();

		MessageQueue::<T>::insert(nonce, message.clone());

		#[cfg(test)]
		mock::mock_lp_gateway_process_success(message);

		#[extrinsic_call]
		process_message(RawOrigin::Signed(caller), nonce);

		Ok(())
	}

	#[benchmark]
	fn process_failed_message() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = account("acc_0", 0, 0);
		let message = LPTestMessage {};
		let error = DispatchError::Unavailable;
		let nonce = T::MessageNonce::one();

		FailedMessageQueue::<T>::insert(nonce, (message.clone(), error));

		#[cfg(test)]
		mock::mock_lp_gateway_process_success(message);

		#[extrinsic_call]
		process_failed_message(RawOrigin::Signed(caller), nonce);

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}
