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

#![cfg_attr(not(feature = "std"), no_std)]

// TODO(william): Add pallet description

#[cfg(test)]
mod mock;

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

	use cfg_traits::{OrderDetails, OrderRatio, TokenSwaps};
	use cfg_types::{
		orders::MuxSwap,
		tokens::{CustomMetadata, LocalAssetId},
	};
	use frame_support::{
		pallet_prelude::{DispatchResult, *},
		traits::{
			fungibles,
			fungibles::Mutate,
			tokens::{Fortitude, Precision, Preservation},
		},
		PalletId,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use orml_traits::asset_registry::{self, Inspect as _};
	use sp_arithmetic::FixedPointOperand;
	use sp_runtime::traits::{AccountIdConversion, EnsureFixedPointNumber, One};

	use super::*;

	type AccountIdFor<T> = <T as frame_system::Config>::AccountId;
	pub type BalanceFor<T> =
		<<T as Config>::Tokens as fungibles::Inspect<AccountIdFor<T>>>::Balance;
	pub type CurrencyFor<T> =
		<<T as Config>::Tokens as fungibles::Inspect<AccountIdFor<T>>>::AssetId;
	pub type OrderIdFor<T> = <<T as Config>::Swaps as TokenSwaps<AccountIdFor<T>>>::OrderId;
	pub type RatioFor<T> = <<T as Config>::Swaps as TokenSwaps<AccountIdFor<T>>>::Ratio;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config
	where
		CurrencyFor<Self>: From<LocalAssetId> + TryInto<LocalAssetId>,
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type AssetRegistry: asset_registry::Inspect<
			CustomMetadata = CustomMetadata,
			AssetId = CurrencyFor<Self>,
		>;

		type Tokens: fungibles::Inspect<Self::AccountId> + Mutate<Self::AccountId>;

		type Swaps: TokenSwaps<Self::AccountId, CurrencyId = CurrencyFor<Self>, Balance = BalanceFor<Self>>
			+ OrderDetails<MuxSwap<CurrencyFor<Self>, RatioFor<Self>>, OrderId = OrderIdFor<Self>>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config>
	where
		CurrencyFor<T>: From<LocalAssetId> + TryInto<LocalAssetId>,
	{
		Deposited {
			who: T::AccountId,
			currency_out: CurrencyFor<T>,
			currency_in: CurrencyFor<T>,
			amount: BalanceFor<T>,
		},
		Burned {
			who: T::AccountId,
			currency_out: CurrencyFor<T>,
			currency_in: CurrencyFor<T>,
			amount: BalanceFor<T>,
		},
		SwapMatched {
			id: OrderIdFor<T>,
			amount: BalanceFor<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The given currency has no metadata set.
		MetadataNotFound,
		/// The given currency has no local representation and can hence not be
		/// deposited to receive a local representation.
		NoLocalRepresentation,
		/// The given currency is not a local currency
		NotLocalCurrency,
		/// The provided local currency does not match the local representation
		/// of the currency to be unlocked
		LocalCurrencyMismatch,
		/// Swap could not be found by id
		SwapNotFound,
		/// Matching orders does only work if there is a one-to-one conversion
		NotIdenticalSwap,
		/// This means the swap is either not a local to variant or not a
		/// variant to local swap
		InvalidSwapCurrencies,
		/// Variant and local representation have mismatching decimals in their
		/// metadata. A conversion between the two is not possible
		DecimalMismatch,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		CurrencyFor<T>: From<LocalAssetId> + TryInto<LocalAssetId>,
		BalanceFor<T>: FixedPointOperand,
	{
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::deposit())]
		pub fn deposit(
			origin: OriginFor<T>,
			currency_out: CurrencyFor<T>,
			amount_out: BalanceFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let local = Self::try_local(&currency_out)?;

			Self::mint_route(&who, local.clone(), currency_out.clone(), amount_out)?;

			Self::deposit_event(Event::<T>::Deposited {
				who,
				currency_out,
				currency_in: local,
				amount: amount_out,
			});

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::burn())]
		pub fn burn(
			origin: OriginFor<T>,
			currency_out: CurrencyFor<T>,
			amount_out: BalanceFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let local = Self::try_local(&currency_out)?;

			Self::burn_route(&who, local.clone(), currency_out.clone(), amount_out)?;

			Self::deposit_event(Event::<T>::Burned {
				who,
				currency_out: local,
				currency_in: currency_out,
				amount: amount_out,
			});

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::match_swap())]
		pub fn match_swap(
			origin: OriginFor<T>,
			order_id: OrderIdFor<T>,
			amount: BalanceFor<T>,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			let order =
				T::Swaps::get_order_details(order_id.clone()).ok_or(Error::<T>::SwapNotFound)?;

			let ratio = match order.ratio {
				OrderRatio::Market => RatioFor::<T>::ensure_from_rational(
					amount,
					T::Swaps::convert_by_market(
						order.currency_in.clone(),
						order.currency_out.clone(),
						amount,
					)?,
				)?,
				OrderRatio::Custom(ratio) => ratio,
			};

			ensure!(ratio == One::one(), Error::<T>::NotIdenticalSwap);

			match (
				Self::try_local(&order.currency_out),
				Self::try_local(&order.currency_in),
			) {
				(Ok(_), Ok(_)) | (Err(_), Err(_)) => {
					return Err(Error::<T>::InvalidSwapCurrencies.into())
				}
				(Ok(local), Err(_)) => {
					ensure!(
						order.currency_in == local,
						Error::<T>::InvalidSwapCurrencies
					);

					T::Tokens::mint_into(local, &Self::account(), amount)?;
					T::Swaps::fill_order(Self::account(), order_id.clone(), amount)?;
				}
				(Err(_), Ok(local)) => {
					ensure!(
						order.currency_out == local,
						Error::<T>::InvalidSwapCurrencies
					);

					T::Swaps::fill_order(Self::account(), order_id.clone(), amount)?;
					T::Tokens::burn_from(
						local,
						&Self::account(),
						amount,
						Precision::Exact,
						Fortitude::Polite,
					)?;
				}
			}

			Self::deposit_event(Event::<T>::SwapMatched {
				id: order_id,
				amount,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T>
	where
		CurrencyFor<T>: From<LocalAssetId> + TryInto<LocalAssetId>,
	{
		fn account() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		fn mint_route(
			who: &T::AccountId,
			local: CurrencyFor<T>,
			variant: CurrencyFor<T>,
			amount: BalanceFor<T>,
		) -> DispatchResult {
			T::Tokens::transfer(
				variant,
				&who,
				&Self::account(),
				amount,
				Preservation::Expendable,
			)?;

			T::Tokens::mint_into(local, &who, amount).map(|_| ())
		}

		fn burn_route(
			who: &T::AccountId,
			local: CurrencyFor<T>,
			variant: CurrencyFor<T>,
			amount: BalanceFor<T>,
		) -> DispatchResult {
			T::Tokens::burn_from(local, &who, amount, Precision::Exact, Fortitude::Polite)?;

			T::Tokens::transfer(
				variant,
				&Self::account(),
				&who,
				amount,
				Preservation::Expendable,
			)
			.map(|_| ())
		}

		fn try_local(currency: &CurrencyFor<T>) -> Result<CurrencyFor<T>, DispatchError> {
			let meta_variant =
				T::AssetRegistry::metadata(currency).ok_or(Error::<T>::MetadataNotFound)?;

			let local: CurrencyFor<T> = meta_variant
				.additional
				.local_representation
				.ok_or(Error::<T>::NoLocalRepresentation)?
				.into();

			let meta_local =
				T::AssetRegistry::metadata(&local).ok_or(Error::<T>::MetadataNotFound)?;

			// NOTE: We could also think about making conversion between local
			//       representations and variants but I fear that we then have problems with
			//       SUM(locked variants) = local. Hence, this restriction.
			ensure!(
				meta_local.decimals == meta_variant.decimals,
				Error::<T>::DecimalMismatch
			);

			Ok(local)
		}
	}
}
