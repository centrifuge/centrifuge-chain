// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// This pallet was made using the ZeitGeist Orderbook pallet as a reference;
// with much of the code being copied or adapted from that pallet.
// The ZeitGeist Orderbook pallet can be found here: https://github.com/zeitgeistpm/zeitgeist/tree/main/zrml/orderbook-v1

#![cfg_attr(not(feature = "std"), no_std)]

//! This module adds an orderbook pallet, allowing orders for currency swaps to
//! be placed and fulfilled for currencies in an asset registry.

#[cfg(test)]
pub(crate) mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

pub use cfg_traits::TokenSwaps;
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use core::fmt::Debug;

	use cfg_traits::TokenSwaps;
	use cfg_types::{investments::Swap, tokens::CustomMetadata};
	use frame_support::{
		pallet_prelude::{DispatchResult, Member, StorageDoubleMap, StorageValue, *},
		traits::{
			fungibles,
			tokens::{AssetId, Precision, Preservation},
		},
		PalletId, Twox64Concat,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use orml_traits::asset_registry::{self, Inspect as _};
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
	use scale_info::TypeInfo;
	use sp_arithmetic::traits::{BaseArithmetic, CheckedSub};
	use sp_runtime::{
		traits::{
			AtLeast32BitUnsigned, EnsureAdd, EnsureDiv, EnsureFixedPointNumber, EnsureMul,
			EnsureSub, MaybeSerializeDeserialize, One, Zero,
		},
		FixedPointNumber, FixedPointOperand,
	};
	use sp_std::cmp::Ordering;

	use super::*;

	type AccountIdFor<T> = <T as frame_system::Config>::AccountId;
	pub type BalanceFor<T> =
		<<T as Config>::Tokens as fungibles::Inspect<AccountIdFor<T>>>::Balance;
	pub type CurrencyFor<T> =
		<<T as Config>::Tokens as fungibles::Inspect<AccountIdFor<T>>>::AssetId;
	pub type OrderIdFor<T> =
		<<T as Config>::Swaps as cfg_traits::TokenSwaps<AccountIdFor<T>>>::OrderId;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type AssetRegistry: asset_registry::Inspect<CustomMetadata = CustomMetadata>;

		type Tokens: fungibles::Inspect<Self::AccountId>
			+ fungibles::InspectHold<Self::AccountId, Reason = ()>
			+ fungibles::MutateHold<Self::AccountId>
			+ fungibles::Mutate<Self::AccountId>;

		type Swaps: TokenSwaps<Self::AccountId, CurrencyId = CurrencyFor<Self>>;

		type Weights: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn deposit(
			origin: OriginFor<T>,
			to_deposit: CurrencyFor<T>,
			amount: BalanceFor<T>,
		) -> DispatchResult {
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn burn(
			origin: OriginFor<T>,
			to_burn: CurrencyFor<T>,
			amount: BalanceFor<T>,
		) -> DispatchResult {
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn match_swap(
			origin: OriginFor<T>,
			order_id: OrderIdFor<T>,
			amount: BalanceFor<T>,
		) -> DispatchResult {
			Ok(())
		}
	}
}
