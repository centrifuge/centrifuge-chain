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
	use cfg_types::{
		investments::Swap,
		tokens::{CustomMetadata, LocalAssetId},
	};
	use frame_support::{
		pallet_prelude::{DispatchResult, Member, StorageDoubleMap, StorageValue, *},
		traits::{
			fungibles,
			fungibles::Mutate,
			tokens::{AssetId, Fortitude, Precision, Preservation},
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
			AccountIdConversion, AtLeast32BitUnsigned, EnsureAdd, EnsureDiv,
			EnsureFixedPointNumber, EnsureMul, EnsureSub, MaybeSerializeDeserialize, One, Zero,
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

		type AssetRegistry: asset_registry::Inspect<
			CustomMetadata = CustomMetadata,
			AssetId = CurrencyFor<Self>,
		>;

		type Tokens: fungibles::Inspect<Self::AccountId> + Mutate<Self::AccountId>;

		type Swaps: TokenSwaps<Self::AccountId, CurrencyId = CurrencyFor<Self>>;

		type Weights: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Deposited {
			who: T::AccountId,
			what: CurrencyFor<T>,
			received: CurrencyFor<T>,
			amount: BalanceFor<T>,
		},
		Burned {
			who: T::AccountId,
			what: CurrencyFor<T>,
			received: CurrencyFor<T>,
			amount: BalanceFor<T>,
		},
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// The given currency has no metadata set.
		MissingMetadata,
		/// The given currency has no local representation and can hence not be
		/// deposited to receive a local representation.
		NoLocalRepresentation,
		/// The given currency is not a local currency
		NotLocalCurrency,
		/// The provided local currency does not match the local representation
		/// of the currency to be unlocked
		LocalCurrencyMismatch,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		CurrencyFor<T>: From<LocalAssetId> + TryInto<LocalAssetId>,
	{
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn deposit(
			origin: OriginFor<T>,
			to_deposit: CurrencyFor<T>,
			amount: BalanceFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let local: CurrencyFor<T> = T::AssetRegistry::metadata(&to_deposit)
				.ok_or(Error::<T>::MissingMetadata)?
				.additional
				.local_representation
				.ok_or(Error::<T>::NoLocalRepresentation)?
				.into();

			T::Tokens::transfer(
				to_deposit.clone(),
				&who,
				&T::PalletId::get().into_account_truncating(),
				amount,
				Preservation::Expendable,
			)?;

			T::Tokens::mint_into(local.clone(), &who, amount)?;

			Self::deposit_event(Event::<T>::Deposited {
				who,
				what: to_deposit,
				received: local,
				amount,
			});

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn burn(
			origin: OriginFor<T>,
			to_burn: CurrencyFor<T>,
			to_receive: CurrencyFor<T>,
			amount: BalanceFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let provide_local_id: LocalAssetId = to_burn
				.clone()
				.try_into()
				.map_err(|_| Error::<T>::NotLocalCurrency)?;

			let needed_local_id = T::AssetRegistry::metadata(&to_receive)
				.ok_or(Error::<T>::MissingMetadata)?
				.additional
				.local_representation
				.ok_or(Error::<T>::NoLocalRepresentation)?;

			ensure!(
				provide_local_id == needed_local_id,
				Error::<T>::LocalCurrencyMismatch
			);

			T::Tokens::burn_from(
				provide_local_id.into(),
				&who,
				amount,
				Precision::Exact,
				Fortitude::Polite,
			)?;

			T::Tokens::transfer(
				to_receive.clone(),
				&T::PalletId::get().into_account_truncating(),
				&who,
				amount,
				Preservation::Expendable,
			)?;

			Self::deposit_event(Event::<T>::Burned {
				who,
				what: to_burn,
				received: to_receive,
				amount,
			});

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
