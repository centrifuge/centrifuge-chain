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

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{OrderManager, TrancheCurrency};
	use cfg_types::{FulfillmentWithPrice, PoolLocator, TotalOrder};
	use frame_support::{
		pallet_prelude::*,
		traits::fungibles::{Inspect, Mutate, Transfer},
	};
	use sp_runtime::{
		traits::{AccountIdConversion, AtLeast32BitUnsigned},
		FixedPointNumber, FixedPointOperand,
	};

	use crate::TEST_PALLET_ID;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type PoolId: Member + Parameter + Default + Copy + MaxEncodedLen;

		type TrancheId: Member + Parameter + Default + Copy + MaxEncodedLen;

		type Balance: Member
			+ Parameter
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ AtLeast32BitUnsigned
			+ MaybeSerializeDeserialize
			+ From<u64>
			+ FixedPointOperand;

		type CurrencyId: Member + Parameter + Copy + MaxEncodedLen + MaybeSerializeDeserialize;

		type InvestmentId: Member
			+ Parameter
			+ Copy
			+ MaxEncodedLen
			+ MaybeSerializeDeserialize
			+ Into<Self::CurrencyId>
			+ TrancheCurrency<Self::PoolId, Self::TrancheId>;

		type Rate: FixedPointNumber<Inner = Self::Balance>;

		type Tokens: Inspect<Self::AccountId, Balance = Self::Balance, AssetId = Self::CurrencyId>
			+ Mutate<Self::AccountId>
			+ Transfer<Self::AccountId>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub invest_orders: Vec<(T::InvestmentId, T::Balance, T::CurrencyId)>,
		pub redeem_orders: Vec<(T::InvestmentId, T::Balance, T::CurrencyId)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				invest_orders: Default::default(),
				redeem_orders: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			for (id, amount, payment_currency) in &self.invest_orders {
				InvestOrders::<T>::insert(*id, TotalOrder { amount: *amount });
				if !PaymentCurrency::<T>::contains_key(id) {
					PaymentCurrency::<T>::insert(*id, *payment_currency);
				}
			}
			for (id, amount, payment_currency) in &self.redeem_orders {
				RedeemOrders::<T>::insert(*id, TotalOrder { amount: *amount });
				if !PaymentCurrency::<T>::contains_key(id) {
					PaymentCurrency::<T>::insert(*id, *payment_currency);
				}
			}
		}
	}

	#[pallet::storage]
	pub type PaymentCurrency<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, T::CurrencyId>;

	#[pallet::storage]
	pub type InvestOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, TotalOrder<T::Balance>, ValueQuery>;

	#[pallet::storage]
	pub type RedeemOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, TotalOrder<T::Balance>, ValueQuery>;

	impl<T: Config> Pallet<T> {}

	impl<T: Config> OrderManager for Pallet<T> {
		type Error = DispatchError;
		type Fulfillment = FulfillmentWithPrice<T::Rate>;
		type InvestmentId = T::InvestmentId;
		type Orders = TotalOrder<T::Balance>;

		/// When called the manager return the current
		/// invest orders for the given investment class.
		fn invest_orders(asset_id: Self::InvestmentId) -> Result<Self::Orders, Self::Error> {
			Ok(InvestOrders::<T>::get(asset_id))
		}

		/// When called the manager return the current
		/// redeem orders for the given investment class.
		fn redeem_orders(asset_id: Self::InvestmentId) -> Result<Self::Orders, Self::Error> {
			Ok(RedeemOrders::<T>::get(asset_id))
		}

		/// Signals the manager that the previously
		/// fetch invest orders for a given investment class
		/// will be fulfilled by fulfillment.
		fn invest_fulfillment(
			asset_id: Self::InvestmentId,
			fulfillment: Self::Fulfillment,
		) -> Result<(), Self::Error> {
			let orders = InvestOrders::<T>::get(asset_id);

			// Move tokens to pools
			let tokens_to_transfer_to_pool = fulfillment.of_amount.mul_floor(orders.amount);
			T::Tokens::mint_into(
				PaymentCurrency::<T>::get(asset_id)
					.expect("PaymentCurrency is provided in testing. Qed."),
				&PoolLocator {
					pool_id: asset_id.of_pool(),
				}
				.into_account_truncating(),
				tokens_to_transfer_to_pool,
			)
			.expect("Minting must work. Qed.");

			// Update local order
			InvestOrders::<T>::insert(
				asset_id,
				TotalOrder {
					amount: orders.amount - tokens_to_transfer_to_pool,
				},
			);

			// Mint tranche tokens into test pallet-id
			let tranche_tokens_to_mint = fulfillment
				.price
				.reciprocal()
				.unwrap()
				.checked_mul_int(tokens_to_transfer_to_pool)
				.unwrap();
			T::Tokens::mint_into(
				asset_id.into(),
				&TEST_PALLET_ID.into_account_truncating(),
				tranche_tokens_to_mint,
			)
			.expect("Minting must work. Qed.");

			Ok(())
		}

		/// Signals the manager that the previously
		/// fetch redeem orders for a given investment class
		/// will be fulfilled by fulfillment.
		fn redeem_fulfillment(
			asset_id: Self::InvestmentId,
			fulfillment: Self::Fulfillment,
		) -> Result<(), Self::Error> {
			let orders = RedeemOrders::<T>::get(asset_id);

			let tokens_to_burn_from_test_pallet = fulfillment.of_amount.mul_floor(orders.amount);
			T::Tokens::burn_from(
				asset_id.into(),
				&TEST_PALLET_ID.into_account_truncating(),
				tokens_to_burn_from_test_pallet,
			)
			.expect("Burning must work. Qed.");

			// Update local order
			// Update local order
			RedeemOrders::<T>::insert(
				asset_id,
				TotalOrder {
					amount: orders.amount - tokens_to_burn_from_test_pallet,
				},
			);

			// Burn payment currency from pool
			let payment_currency_to_burn = fulfillment
				.price
				.checked_mul_int(tokens_to_burn_from_test_pallet)
				.unwrap();
			T::Tokens::burn_from(
				PaymentCurrency::<T>::get(asset_id)
					.expect("PaymentCurrency is provided in testing. Qed."),
				&PoolLocator {
					pool_id: asset_id.of_pool(),
				}
				.into_account_truncating(),
				payment_currency_to_burn,
			)
			.expect("Minting must work. Qed.");

			Ok(())
		}
	}
}
