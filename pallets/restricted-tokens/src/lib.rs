// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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
#![cfg_attr(not(feature = "std"), no_std)]

pub use impl_currency::*;
pub use impl_fungible::*;
pub use impl_fungibles::*;
///! A crate that allows for checking of preconditions before sending tokens.
///! Mimics ORML-tokens Call-Api.
pub use pallet::*;
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod impl_currency;
mod impl_fungible;
mod impl_fungibles;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod weights;

use frame_support::traits::{fungible, fungibles};
use frame_support::traits::{Currency, LockableCurrency, ReservableCurrency};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use scale_info::TypeInfo;

pub enum TokenType {
	Native,
	Other,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub struct TransferDetails<AccountId, CurrencyId, Balance> {
	pub send: AccountId,
	pub recv: AccountId,
	pub id: CurrencyId,
	pub amount: Balance,
}

impl<AccountId, CurrencyId, Balance> TransferDetails<AccountId, CurrencyId, Balance> {
	pub fn new(send: AccountId, recv: AccountId, id: CurrencyId, amount: Balance) -> Self {
		TransferDetails {
			send,
			recv,
			id,
			amount,
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::impl_currency::{CurrencyEffects, ReservableCurrencyEffects};
	use crate::impl_fungible::{
		FungibleInspectEffects, FungibleInspectHoldEffects, FungibleMutateEffects,
		FungibleMutateHoldEffects, FungibleTransferEffects,
	};
	use crate::impl_fungibles::{
		FungiblesInspectEffects, FungiblesInspectHoldEffects, FungiblesMutateEffects,
		FungiblesMutateHoldEffects, FungiblesTransferEffects,
	};
	use common_traits::{PreConditions, TokenMetadata};
	use frame_support::scale_info::TypeInfo;
	use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, CheckedAdd, StaticLookup};
	use frame_support::sp_runtime::ArithmeticError;
	use frame_system::pallet_prelude::*;
	use sp_std::cmp::max;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The balance type
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The currency-id of this pallet
		type CurrencyId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ TokenMetadata;

		/// Checks the pre conditions for every transfer via the user api (i.e. extrinsics)
		type PreExtrTransfer: PreConditions<
			TransferDetails<Self::AccountId, Self::CurrencyId, Self::Balance>,
			Result = bool,
		>;

		/// Checks the pre conditions for trait fungibles::Inspect calls
		type PreFungiblesInspect: PreConditions<
			FungiblesInspectEffects<Self::CurrencyId, Self::AccountId, Self::Balance>,
			Result = Self::Balance,
		>;

		/// Checks the pre conditions for trait fungibles::InspectHold calls
		type PreFungiblesInspectHold: PreConditions<
			FungiblesInspectHoldEffects<Self::CurrencyId, Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		/// Checks the pre conditions for trait fungibles::Mutate calls
		type PreFungiblesMutate: PreConditions<
			FungiblesMutateEffects<Self::CurrencyId, Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		/// Checks the pre conditions for trait fungibles::MutateHold calls
		type PreFungiblesMutateHold: PreConditions<
			FungiblesMutateHoldEffects<Self::CurrencyId, Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		/// Checks the pre conditions for trait fungibles::Transfer calls
		type PreFungiblesTransfer: PreConditions<
			FungiblesTransferEffects<Self::CurrencyId, Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		type Fungibles: fungibles::Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>
			+ fungibles::InspectHold<Self::AccountId>
			+ fungibles::Mutate<Self::AccountId>
			+ fungibles::MutateHold<Self::AccountId>
			+ fungibles::Transfer<Self::AccountId>;

		/// Checks the pre conditions for trait Currency calls
		type PreCurrency: PreConditions<
			CurrencyEffects<Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		/// Checks the pre conditions for trait ReservableCurrency calls
		type PreReservableCurrency: PreConditions<
			ReservableCurrencyEffects<Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		/// Checks the pre conditions for trait fungible::Inspect calls
		type PreFungibleInspect: PreConditions<
			FungibleInspectEffects<Self::AccountId, Self::Balance>,
			Result = Self::Balance,
		>;

		/// Checks the pre conditions for trait fungible::InspectHold calls
		type PreFungibleInspectHold: PreConditions<
			FungibleInspectHoldEffects<Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		/// Checks the pre conditions for trait fungible::Mutate calls
		type PreFungibleMutate: PreConditions<
			FungibleMutateEffects<Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		/// Checks the pre conditions for trait fungible::MutateHold calls
		type PreFungibleMutateHold: PreConditions<
			FungibleMutateHoldEffects<Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		/// Checks the pre conditions for trait fungible::Transfer calls
		type PreFungibleTransfer: PreConditions<
			FungibleTransferEffects<Self::AccountId, Self::Balance>,
			Result = bool,
		>;

		type NativeFungible: Currency<Self::AccountId, Balance = Self::Balance>
			+ LockableCurrency<Self::AccountId>
			+ ReservableCurrency<Self::AccountId>
			+ fungible::Inspect<Self::AccountId, Balance = Self::Balance>
			+ fungible::InspectHold<Self::AccountId>
			+ fungible::Mutate<Self::AccountId>
			+ fungible::MutateHold<Self::AccountId>
			+ fungible::Transfer<Self::AccountId>;

		type NativeToken: Get<Self::CurrencyId>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Transfer succeeded.
		Transfer {
			currency_id: T::CurrencyId,
			from: T::AccountId,
			to: T::AccountId,
			amount: T::Balance,
		},
		/// A balance was set by root.
		BalanceSet {
			currency_id: T::CurrencyId,
			who: T::AccountId,
			free: T::Balance,
			reserved: T::Balance,
		},
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		PreConditionsNotMet,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(max(T::WeightInfo::transfer_native(), T::WeightInfo::transfer_other()))]
		pub fn transfer(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;

			ensure!(
				T::PreExtrTransfer::check(TransferDetails::new(
					from.clone(),
					to.clone(),
					currency_id,
					amount
				)),
				Error::<T>::PreConditionsNotMet
			);

			let token = if T::NativeToken::get() == currency_id {
				<T::NativeFungible as fungible::Transfer<T::AccountId>>::transfer(
					&from, &to, amount, false,
				)?;

				TokenType::Native
			} else {
				<T::Fungibles as fungibles::Transfer<T::AccountId>>::transfer(
					currency_id,
					&from,
					&to,
					amount,
					false,
				)?;

				TokenType::Other
			};

			Self::deposit_event(Event::Transfer {
				currency_id,
				from,
				to,
				amount,
			});

			match token {
				TokenType::Native => Ok(Some(T::WeightInfo::transfer_native()).into()),
				TokenType::Other => Ok(Some(T::WeightInfo::transfer_other()).into()),
			}
		}

		#[pallet::weight(max(
			T::WeightInfo::transfer_keep_alive_native(),
			T::WeightInfo::transfer_keep_alive_other(),
		))]
		pub fn transfer_keep_alive(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;

			ensure!(
				T::PreExtrTransfer::check(TransferDetails::new(
					from.clone(),
					to.clone(),
					currency_id,
					amount
				)),
				Error::<T>::PreConditionsNotMet
			);

			let token = if T::NativeToken::get() == currency_id {
				<T::NativeFungible as fungible::Transfer<T::AccountId>>::transfer(
					&from, &to, amount, true,
				)?;

				TokenType::Native
			} else {
				<T::Fungibles as fungibles::Transfer<T::AccountId>>::transfer(
					currency_id,
					&from,
					&to,
					amount,
					true,
				)?;

				TokenType::Other
			};

			Self::deposit_event(Event::Transfer {
				currency_id,
				from,
				to,
				amount,
			});

			match token {
				TokenType::Native => Ok(Some(T::WeightInfo::transfer_keep_alive_native()).into()),
				TokenType::Other => Ok(Some(T::WeightInfo::transfer_keep_alive_other()).into()),
			}
		}

		#[pallet::weight(max(
			T::WeightInfo::transfer_all_native(),
			T::WeightInfo::transfer_all_other(),
		))]
		pub fn transfer_all(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			keep_alive: bool,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;

			let reducible_balance = if T::NativeToken::get() == currency_id {
				<T::NativeFungible as fungible::Inspect<T::AccountId>>::reducible_balance(
					&from, keep_alive,
				)
			} else {
				<T::Fungibles as fungibles::Inspect<T::AccountId>>::reducible_balance(
					currency_id,
					&from,
					keep_alive,
				)
			};

			ensure!(
				T::PreExtrTransfer::check(TransferDetails::new(
					from.clone(),
					to.clone(),
					currency_id,
					reducible_balance
				)),
				Error::<T>::PreConditionsNotMet
			);

			let token = if T::NativeToken::get() == currency_id {
				<T::NativeFungible as fungible::Transfer<T::AccountId>>::transfer(
					&from,
					&to,
					reducible_balance,
					keep_alive,
				)?;

				TokenType::Native
			} else {
				<T::Fungibles as fungibles::Transfer<T::AccountId>>::transfer(
					currency_id,
					&from,
					&to,
					reducible_balance,
					keep_alive,
				)?;

				TokenType::Other
			};

			Self::deposit_event(Event::Transfer {
				currency_id,
				from,
				to,
				amount: reducible_balance,
			});

			match token {
				TokenType::Native => Ok(Some(T::WeightInfo::transfer_all_native()).into()),
				TokenType::Other => Ok(Some(T::WeightInfo::transfer_all_other()).into()),
			}
		}

		#[pallet::weight(max(
			T::WeightInfo::force_transfer_native(),
			T::WeightInfo::force_transfer_other(),
		))]
		pub fn force_transfer(
			origin: OriginFor<T>,
			source: <T::Lookup as StaticLookup>::Source,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let from = T::Lookup::lookup(source)?;
			let to = T::Lookup::lookup(dest)?;

			let token = if T::NativeToken::get() == currency_id {
				<T::NativeFungible as fungible::Transfer<T::AccountId>>::transfer(
					&from, &to, amount, false,
				)?;

				TokenType::Native
			} else {
				<T::Fungibles as fungibles::Transfer<T::AccountId>>::transfer(
					currency_id,
					&from,
					&to,
					amount,
					false,
				)?;

				TokenType::Other
			};

			Self::deposit_event(Event::Transfer {
				currency_id,
				from,
				to,
				amount,
			});

			match token {
				TokenType::Native => Ok(Some(T::WeightInfo::force_transfer_native()).into()),
				TokenType::Other => Ok(Some(T::WeightInfo::force_transfer_other()).into()),
			}
		}

		#[pallet::weight(max(
			T::WeightInfo::set_balance_native(),
			T::WeightInfo::set_balance_other(),
		))]
		pub fn set_balance(
			origin: OriginFor<T>,
			who: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] new_free: T::Balance,
			#[pallet::compact] new_reserved: T::Balance,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let who = T::Lookup::lookup(who)?;

			let new_total = new_free
				.checked_add(&new_reserved)
				.ok_or(ArithmeticError::Overflow)?;

			let token = if T::NativeToken::get() == currency_id {
				let old_reserved =
					<T::NativeFungible as fungible::InspectHold<T::AccountId>>::balance_on_hold(
						&who,
					);
				<T::NativeFungible as fungible::MutateHold<T::AccountId>>::release(
					&who,
					old_reserved,
					false,
				)?;
				let to_burn = <T::NativeFungible as fungible::Inspect<T::AccountId>>::balance(&who);
				<T::NativeFungible as fungible::Mutate<T::AccountId>>::burn_from(&who, to_burn)?;
				<T::NativeFungible as fungible::Mutate<T::AccountId>>::mint_into(&who, new_total)?;
				<T::NativeFungible as fungible::MutateHold<T::AccountId>>::hold(
					&who,
					new_reserved,
				)?;

				TokenType::Native
			} else {
				let old_reserved =
					<T::Fungibles as fungibles::InspectHold<T::AccountId>>::balance_on_hold(
						currency_id,
						&who,
					);
				<T::Fungibles as fungibles::MutateHold<T::AccountId>>::release(
					currency_id,
					&who,
					old_reserved,
					false,
				)?;
				let to_burn =
					<T::Fungibles as fungibles::Inspect<T::AccountId>>::balance(currency_id, &who);
				<T::Fungibles as fungibles::Mutate<T::AccountId>>::burn_from(
					currency_id,
					&who,
					to_burn,
				)?;
				<T::Fungibles as fungibles::Mutate<T::AccountId>>::mint_into(
					currency_id,
					&who,
					new_total,
				)?;
				<T::Fungibles as fungibles::MutateHold<T::AccountId>>::hold(
					currency_id,
					&who,
					new_reserved,
				)?;

				TokenType::Other
			};

			Self::deposit_event(Event::BalanceSet {
				currency_id,
				who: who,
				free: new_free,
				reserved: new_reserved,
			});

			match token {
				TokenType::Native => Ok(Some(T::WeightInfo::set_balance_native()).into()),
				TokenType::Other => Ok(Some(T::WeightInfo::set_balance_other()).into()),
			}
		}
	}
}
