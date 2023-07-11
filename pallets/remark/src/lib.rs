// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Utility Pallet
//! A stateless pallet with helpers for dispatch management which does no
//! re-authentication.
//!
//! - [`Config`]
//! - [`Call`]
//!
//! ## Overview
//!
//! This pallet contains two basic pieces of functionality:
//! - Batch dispatch: A stateless operation, allowing any origin to execute
//!   multiple calls in a single dispatch. This can be useful to amalgamate
//!   proposals, combining `set_code` with corresponding `set_storage`s, for
//!   efficient multiple payouts with just a single signature verify, or in
//!   combination with one of the other two dispatch functionality.
//! - Pseudonymal dispatch: A stateless operation, allowing a signed origin to
//!   execute a call from an alternative signed origin. Each account has 2 *
//!   2**16 possible "pseudonyms" (alternative account IDs) and these can be
//!   stacked. This can be useful as a key management tool, where you need
//!   multiple distinct accounts (e.g. as controllers for many staking
//!   accounts), but where it's perfectly fine to have each of them controlled
//!   by the same underlying keypair. Derivative accounts are, for the purposes
//!   of proxy filtering considered exactly the same as the origin and are thus
//!   hampered with the origin's filters.
//!
//! Since proxy filters are respected in all dispatches of this pallet, it
//! should never need to be filtered by any proxy.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! #### For batch dispatch
//! * `batch` - Dispatch multiple calls from the sender's origin.
//!
//! #### For pseudonymal dispatch
//! * `as_derivative` - Dispatch a call from a derivative signed origin.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

mod benchmarking;
mod tests;
pub mod weights;

use codec::{Decode, Encode};
use frame_support::{
	dispatch::{extract_actual_weight, GetDispatchInfo, PostDispatchInfo},
	traits::{IsSubType, OriginTrait, UnfilteredDispatchable},
};
pub use pallet::*;
use sp_core::TypeId;
use sp_io::hashing::blake2_256;
use sp_runtime::traits::{BadOrigin, Dispatchable, TrailingZeroInput};
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configuration trait.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Remark: Parameter;

		/// The overarching call type.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>
			+ UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ IsSubType<Call<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeCall>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event {
		/// Batch of dispatches completed fully with no error.
		Remark {
			remark: T::Remark,
			calls: Vec<<T as Config>::RuntimeCall>,
		},
		/// A single item within a Batch of dispatches has completed with no
		/// error.
		ItemCompleted {
			remark: T::Remark,
			call: <T as Config>::RuntimeCall,
		},
	}

	// Align the call size to 1KB. As we are currently compiling the runtime for
	// native/wasm the `size_of` of the `Call` can be different. To ensure that this
	// don't leads to mismatches between native/wasm or to different metadata for
	// the same runtime, we algin the call size. The value is chosen big enough to
	// hopefully never reach it.
	const CALL_ALIGN: u32 = 1024;

	#[pallet::extra_constants]
	impl<T: Config> Pallet<T> {
		/// The limit on the number of batched calls.
		fn batched_calls_limit() -> u32 {
			let allocator_limit = sp_core::MAX_POSSIBLE_ALLOCATION;
			let call_size = ((sp_std::mem::size_of::<<T as Config>::RuntimeCall>() as u32
				+ CALL_ALIGN - 1)
				/ CALL_ALIGN) * CALL_ALIGN;
			// The margin to take into account vec doubling capacity.
			let margin_factor = 3;

			allocator_limit / margin_factor / call_size
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			// If you hit this error, you need to try to `Box` big dispatchable parameters.
			assert!(
				sp_std::mem::size_of::<<T as Config>::RuntimeCall>() as u32 <= CALL_ALIGN,
				"Call enum size should be smaller than {} bytes.",
				CALL_ALIGN,
			);
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Too many calls batched.
		TooManyCalls,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(2)]
		#[pallet::weight({
			let dispatch_infos = calls.iter().map(|call| call.get_dispatch_info()).collect::<Vec<_>>();
			let dispatch_weight = dispatch_infos.iter()
				.map(|di| di.weight)
				.fold(Weight::zero(), |total: Weight, weight: Weight| total.saturating_add(weight))
				.saturating_add(T::WeightInfo::batch_all(calls.len() as u32));
			let dispatch_class = {
				let all_operational = dispatch_infos.iter()
					.map(|di| di.class)
					.all(|class| class == DispatchClass::Operational);
				if all_operational {
					DispatchClass::Operational
				} else {
					DispatchClass::Normal
				}
			};
			(dispatch_weight, dispatch_class)
		})]
		pub fn remark(
			origin: OriginFor<T>,
			remark: T::Remark,
			calls: Vec<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			// Do not allow the `None` origin.
			if ensure_none(origin.clone()).is_ok() {
				return Err(BadOrigin.into());
			}

			let is_root = ensure_root(origin.clone()).is_ok();
			let calls_len = calls.len();
			ensure!(
				calls_len <= Self::batched_calls_limit() as usize,
				Error::<T>::TooManyCalls
			);

			// Track the actual weight of each of the batch calls.
			let mut weight = Weight::zero();
			for (index, call) in calls.into_iter().enumerate() {
				let info = call.get_dispatch_info();
				// If origin is root, bypass any dispatch filter; root can call anything.
				let result = if is_root {
					call.dispatch_bypass_filter(origin.clone())
				} else {
					let mut filtered_origin = origin.clone();
					// Don't allow users to nest `batch_all` calls.
					filtered_origin.add_filter(
						move |c: &<T as frame_system::Config>::RuntimeCall| {
							let c = <T as Config>::RuntimeCall::from_ref(c);
							!matches!(c.is_sub_type(), Some(Call::batch_all { .. }))
						},
					);
					call.dispatch(filtered_origin)
				};
				// Add the weight of this call.
				weight = weight.saturating_add(extract_actual_weight(&result, &info));
				result.map_err(|mut err| {
					// Take the weight of this function itself into account.
					let base_weight = T::WeightInfo::batch_all(index.saturating_add(1) as u32);
					// Return the actual used weight + base_weight of this call.
					err.post_info = Some(base_weight + weight).into();
					err
				})?;
				Self::deposit_event(Event::ItemCompleted { remark, call });
			}
			Self::deposit_event(Event::RemarkCompleted { remark, calls });
			let base_weight = T::WeightInfo::batch_all(calls_len as u32);
			Ok(Some(base_weight.saturating_add(weight)).into())
		}
	}
}
