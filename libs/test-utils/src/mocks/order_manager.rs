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
	use cfg_traits::investments::{
		Investment, InvestmentAccountant, InvestmentProperties, OrderManager, TrancheCurrency,
	};
	use cfg_types::orders::{FulfillmentWithPrice, TotalOrder};
	use frame_support::{
		pallet_prelude::*,
		traits::fungibles::{Inspect, Mutate},
		PalletId,
	};
	use frame_support::traits::tokens::Preservation;
	use frame_system::pallet_prelude::BlockNumberFor;
	use sp_runtime::{traits::AccountIdConversion, FixedPointNumber, FixedPointOperand};

	pub struct OrderManagerAccount;

	impl OrderManagerAccount {
		pub const LOCAL_ID: PalletId = PalletId(*b"OrdrMngr");

		pub fn get<T: frame_system::Config>() -> T::AccountId {
			OrderManagerAccount::LOCAL_ID.into_account_truncating()
		}
	}

	type BalanceOf<T> =
		<<T as Config>::Tokens as Inspect<<T as frame_system::Config>::AccountId>>::Balance;
	type CurrencyOf<T> =
		<<T as Config>::Tokens as Inspect<<T as frame_system::Config>::AccountId>>::AssetId;

	#[pallet::config]
	pub trait Config: frame_system::Config
	where
		<Self::Tokens as Inspect<Self::AccountId>>::Balance:
			From<u64> + FixedPointOperand + MaxEncodedLen + MaybeSerializeDeserialize,
		<Self::Tokens as Inspect<Self::AccountId>>::AssetId:
			MaxEncodedLen + MaybeSerializeDeserialize,
		<Self::Accountant as InvestmentAccountant<Self::AccountId>>::InvestmentInfo:
			InvestmentProperties<Self::AccountId, Currency = CurrencyOf<Self>>,
	{
		type FundsAccount: Get<PalletId>;

		type Accountant: InvestmentAccountant<
			Self::AccountId,
			Amount = BalanceOf<Self>,
			Error = DispatchError,
			InvestmentId = Self::InvestmentId,
		>;

		type PoolId: Member + Parameter + Default + Copy + MaxEncodedLen;

		type TrancheId: Member + Parameter + Default + Copy + MaxEncodedLen;

		type InvestmentId: Member
			+ Parameter
			+ Copy
			+ MaxEncodedLen
			+ MaybeSerializeDeserialize
			+ Into<CurrencyOf<Self>>
			+ TrancheCurrency<Self::PoolId, Self::TrancheId>;

		type Rate: FixedPointNumber<Inner = BalanceOf<Self>>;

		type Tokens: Inspect<Self::AccountId> + Mutate<Self::AccountId>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config>
	where
		<T::Tokens as Inspect<T::AccountId>>::Balance:
			From<u64> + FixedPointOperand + MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Tokens as Inspect<T::AccountId>>::AssetId: MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		pub invest_orders: Vec<(T::InvestmentId, BalanceOf<T>)>,
		pub redeem_orders: Vec<(T::InvestmentId, BalanceOf<T>)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T>
	where
		<T::Tokens as Inspect<T::AccountId>>::Balance:
			From<u64> + FixedPointOperand + MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Tokens as Inspect<T::AccountId>>::AssetId: MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		fn default() -> Self {
			Self {
				invest_orders: Default::default(),
				redeem_orders: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T>
	where
		<T::Tokens as Inspect<T::AccountId>>::Balance:
			From<u64> + FixedPointOperand + MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Tokens as Inspect<T::AccountId>>::AssetId: MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		fn build(&self) {
			for (id, amount) in &self.invest_orders {
				InvestOrders::<T>::insert(*id, TotalOrder { amount: *amount });
			}
			for (id, amount) in &self.redeem_orders {
				RedeemOrders::<T>::insert(*id, TotalOrder { amount: *amount });
			}
		}
	}

	#[pallet::storage]
	pub type InvestOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, TotalOrder<BalanceOf<T>>>;

	#[pallet::storage]
	pub type RedeemOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, TotalOrder<BalanceOf<T>>>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
	where
		<T::Tokens as Inspect<T::AccountId>>::Balance:
			From<u64> + FixedPointOperand + MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Tokens as Inspect<T::AccountId>>::AssetId: MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		// TODO: Remove once we are on Substrate:polkadot-v0.9.29
	}
	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T::Tokens as Inspect<T::AccountId>>::Balance:
			From<u64> + FixedPointOperand + MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Tokens as Inspect<T::AccountId>>::AssetId: MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		// TODO: Remove once we are on Substrate:polkadot-v0.9.29
	}

	impl<T: Config> Pallet<T>
	where
		<T::Tokens as Inspect<T::AccountId>>::Balance:
			From<u64> + FixedPointOperand + MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Tokens as Inspect<T::AccountId>>::AssetId: MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		/// **Test Method**
		///
		/// Moves funds from the `T::FundsAccount` to the local
		/// `OrderManagerAccount`
		pub fn update_invest_order(
			investment_id: T::InvestmentId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let mut orders = InvestOrders::<T>::get(investment_id).unwrap_or_default();
			orders.amount += amount.clone();
			InvestOrders::<T>::insert(investment_id, orders);

			let details = T::Accountant::info(investment_id)?;

			T::Tokens::transfer(
				details.payment_currency(),
				&T::FundsAccount::get().into_account_truncating(),
				&OrderManagerAccount::get::<T>(),
				amount,
				Preservation::Expendable,
			)
			.map(|_| ())
		}

		/// **Test Method**
		///
		/// DOES NOT move funds. We assume that all received `TrancheTokens`
		/// stay in the given `OrderManagerAccount` while testing. Hence, if
		/// redeemptions should be locked we do not need to move them.
		pub fn update_redeem_order(
			investment_id: T::InvestmentId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let mut orders = RedeemOrders::<T>::get(investment_id).unwrap_or_default();
			orders.amount += amount;
			RedeemOrders::<T>::insert(investment_id, orders);

			// NOTE: TrancheTokens NEVER leave the TEST_PALLET_ID account and hence we can
			// keep them here and need no transfer.

			Ok(())
		}
	}

	impl<T: Config> Investment<T::AccountId> for Pallet<T>
	where
		<T::Tokens as Inspect<T::AccountId>>::Balance:
			From<u64> + FixedPointOperand + MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Tokens as Inspect<T::AccountId>>::AssetId: MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		type Amount = BalanceOf<T>;
		type CurrencyId = CurrencyOf<T>;
		type Error = DispatchError;
		type InvestmentId = T::InvestmentId;

		fn update_investment(
			_: &T::AccountId,
			investment_id: Self::InvestmentId,
			amount: Self::Amount,
		) -> Result<(), Self::Error> {
			Self::update_invest_order(investment_id, amount)
		}

		fn accepted_payment_currency(
			investment_id: Self::InvestmentId,
			currency: Self::CurrencyId,
		) -> bool {
			T::Accountant::info(investment_id)
				.map(|info| info.payment_currency() == currency)
				.unwrap_or(false)
		}

		fn investment(
			_: &T::AccountId,
			investment_id: Self::InvestmentId,
		) -> Result<Self::Amount, Self::Error> {
			Ok(InvestOrders::<T>::get(investment_id)
				.unwrap_or_default()
				.amount)
		}

		fn update_redemption(
			_: &T::AccountId,
			investment_id: Self::InvestmentId,
			amount: Self::Amount,
		) -> Result<(), Self::Error> {
			Self::update_redeem_order(investment_id, amount)
		}

		fn accepted_payout_currency(
			investment_id: Self::InvestmentId,
			currency: Self::CurrencyId,
		) -> bool {
			T::Accountant::info(investment_id)
				.map(|info| info.payment_currency() == currency)
				.unwrap_or(false)
		}

		fn redemption(
			_: &T::AccountId,
			investment_id: Self::InvestmentId,
		) -> Result<Self::Amount, Self::Error> {
			Ok(RedeemOrders::<T>::get(investment_id)
				.unwrap_or_default()
				.amount)
		}

		fn investment_requires_collect(
			_investor: &T::AccountId,
			_investment_id: Self::InvestmentId,
		) -> bool {
			unimplemented!("not needed here, could also default to false")
		}

		fn redemption_requires_collect(
			_investor: &T::AccountId,
			_investment_id: Self::InvestmentId,
		) -> bool {
			unimplemented!("not needed here, could also default to false")
		}
	}

	impl<T: Config> OrderManager for Pallet<T>
	where
		<T::Tokens as Inspect<T::AccountId>>::Balance:
			From<u64> + FixedPointOperand + MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Tokens as Inspect<T::AccountId>>::AssetId: MaxEncodedLen + MaybeSerializeDeserialize,
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		type Error = DispatchError;
		type Fulfillment = FulfillmentWithPrice<T::Rate>;
		type InvestmentId = T::InvestmentId;
		type Orders = TotalOrder<BalanceOf<T>>;

		/// When called the manager return the current
		/// invest orders for the given investment class.
		fn process_invest_orders(
			asset_id: Self::InvestmentId,
		) -> Result<Self::Orders, Self::Error> {
			Ok(InvestOrders::<T>::get(asset_id).unwrap_or_default())
		}

		/// When called the manager return the current
		/// redeem orders for the given investment class.
		fn process_redeem_orders(
			asset_id: Self::InvestmentId,
		) -> Result<Self::Orders, Self::Error> {
			Ok(RedeemOrders::<T>::get(asset_id).unwrap_or_default())
		}

		fn invest_orders(asset_id: Self::InvestmentId) -> Self::Orders {
			InvestOrders::<T>::get(asset_id).unwrap_or_default()
		}

		fn redeem_orders(asset_id: Self::InvestmentId) -> Self::Orders {
			RedeemOrders::<T>::get(asset_id).unwrap_or_default()
		}

		/// Signals the manager that the previously
		/// fetch invest orders for a given investment class
		/// will be fulfilled by fulfillment.
		fn invest_fulfillment(
			asset_id: Self::InvestmentId,
			fulfillment: Self::Fulfillment,
		) -> Result<(), Self::Error> {
			let orders = InvestOrders::<T>::get(asset_id).unwrap_or_default();
			InvestOrders::<T>::insert(asset_id, TotalOrder::default());

			// Move tokens to pools
			let tokens_to_transfer_to_pool = fulfillment.of_amount.mul_floor(orders.amount);
			let details = T::Accountant::info(asset_id)?;
			T::Tokens::transfer(
				details.payment_currency(),
				&OrderManagerAccount::get::<T>(),
				&details.payment_account(),
				tokens_to_transfer_to_pool.clone(),
				Preservation::Preserve,
			)
			.expect("Transferring must work. Qed.");

			// Update local order
			InvestOrders::<T>::insert(
				asset_id,
				TotalOrder {
					amount: orders.amount.clone() - tokens_to_transfer_to_pool.clone(),
				},
			);

			// Mint tranche tokens into test pallet-id
			let tranche_tokens_to_mint = fulfillment
				.price
				.reciprocal()
				.unwrap()
				.checked_mul_int(tokens_to_transfer_to_pool)
				.unwrap();
			T::Accountant::deposit(
				&OrderManagerAccount::get::<T>(),
				asset_id,
				tranche_tokens_to_mint,
			)
			.expect("Depositing must work. Qed.");

			Ok(())
		}

		/// Signals the manager that the previously
		/// fetch redeem orders for a given investment class
		/// will be fulfilled by fulfillment.
		fn redeem_fulfillment(
			asset_id: Self::InvestmentId,
			fulfillment: Self::Fulfillment,
		) -> Result<(), Self::Error> {
			let orders = RedeemOrders::<T>::get(asset_id).unwrap_or_default();
			RedeemOrders::<T>::insert(asset_id, TotalOrder::default());

			let tranche_tokens_to_burn_from_test_pallet =
				fulfillment.of_amount.mul_floor(orders.amount);
			T::Accountant::withdraw(
				&OrderManagerAccount::get::<T>(),
				asset_id,
				tranche_tokens_to_burn_from_test_pallet,
			)
			.expect("Withdrawing must work. Qed.");

			// Update local order
			RedeemOrders::<T>::insert(
				asset_id,
				TotalOrder {
					amount: orders.amount - tranche_tokens_to_burn_from_test_pallet,
				},
			);

			let payment_currency_to_move_to_order_manager = fulfillment
				.price
				.checked_mul_int(tranche_tokens_to_burn_from_test_pallet)
				.unwrap();
			let details = T::Accountant::info(asset_id)?;
			T::Tokens::transfer(
				details.payment_currency(),
				&details.payment_account(),
				&OrderManagerAccount::get::<T>(),
				payment_currency_to_move_to_order_manager,
				Preservation::Expendable,
			)
			.expect("Transferring must work. Qed.");

			Ok(())
		}
	}
}
