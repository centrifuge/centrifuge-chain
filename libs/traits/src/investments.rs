// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use sp_std::fmt::Debug;

/// A trait for converting from a PoolId and a TranchId
/// into a given Self::Currency
pub trait TrancheCurrency<PoolId, TrancheId> {
	fn generate(pool_id: PoolId, tranche_id: TrancheId) -> Self;

	fn of_pool(&self) -> PoolId;

	fn of_tranche(&self) -> TrancheId;
}

/// A trait, when implemented allows to invest into
/// investment classes
pub trait Investment<AccountId> {
	type Amount;
	type TrancheAmount;
	type CurrencyId;
	type Error: Debug;
	type InvestmentId;

	/// Updates the current investment amount of who into the
	/// investment class to amount.
	/// Meaning: if amount < previous investment, then investment
	/// will be reduced, and increases in the opposite case.
	fn update_investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Returns, if possible, the currently unprocessed investment amount (in
	/// pool currency) of who into the given investment class.
	///
	/// NOTE: If the investment was (partially) processed, the unprocessed
	/// amount is only updated upon collecting.
	fn investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Amount, Self::Error>;

	/// Updates the current redemption amount (in tranche tokens) of who into
	/// the investment class to amount.
	/// Meaning: if amount < previous redemption, then the redemption
	/// will be reduced, and increased in the opposite case.
	///
	/// NOTE: Redemptions are bound by the processed investment amount.
	fn update_redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::TrancheAmount,
	) -> Result<(), Self::Error>;

	/// Returns, if possible, the currently unprocessed redemption amount (in
	/// tranche tokens) of who into the given investment class.
	///
	/// NOTE: If the redemption was (partially) processed, the unprocessed
	/// amount is only updated upon collecting.
	fn redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::TrancheAmount, Self::Error>;

	/// Checks whether an investment requires to be collected before it can be
	/// updated.
	///
	/// NOTE: Defaults to false if the investment does not exist.
	fn investment_requires_collect(investor: &AccountId, investment_id: Self::InvestmentId)
		-> bool;

	/// Checks whether a redemption requires to be collected before it can be
	/// further updated.
	///
	/// NOTE: Defaults to false if the redemption does not exist.
	fn redemption_requires_collect(investor: &AccountId, investment_id: Self::InvestmentId)
		-> bool;
}

/// A trait which allows to collect existing investments and redemptions.
pub trait InvestmentCollector<AccountId> {
	type Error: Debug;
	type InvestmentId;
	type Result;

	/// Collect the results of a user's invest orders for the given
	/// investment. If any amounts are not fulfilled they are directly
	/// appended to the next active order for this investment.
	fn collect_investment(
		who: AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Result, Self::Error>;

	/// Collect the results of a users redeem orders for the given
	/// investment. If any amounts are not fulfilled they are directly
	/// appended to the next active order for this investment.
	fn collect_redemption(
		who: AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Result, Self::Error>;
}

/// A trait, when implemented must take care of
/// collecting orders (invest & redeem) for a given investment class.
/// When being asked it must return the current orders and
/// when being singled about a fulfillment, it must act accordingly.
pub trait OrderManager {
	type Error;
	type InvestmentId;
	type Orders;
	type Fulfillment;

	/// When called the manager return the current
	/// invest orders for the given investment class.
	fn invest_orders(asset_id: Self::InvestmentId) -> Self::Orders;

	/// When called the manager return the current
	/// redeem orders for the given investment class.
	fn redeem_orders(asset_id: Self::InvestmentId) -> Self::Orders;

	/// When called the manager return the current
	/// invest orders for the given investment class.
	/// Callers of this method can expect that the returned
	/// orders equal the returned orders from `invest_orders`.
	///
	/// **NOTE:** Once this is called, the OrderManager is expected
	/// to start a new round of orders and return an error if this
	/// method is to be called again before `invest_fulfillment` is
	/// called.
	fn process_invest_orders(asset_id: Self::InvestmentId) -> Result<Self::Orders, Self::Error>;

	/// When called the manager return the current
	/// invest orders for the given investment class.
	/// Callers of this method can expect that the returned
	/// orders equal the returned orders from `redeem_orders`.
	///
	/// **NOTE:** Once this is called, the OrderManager is expected
	/// to start a new round of orders and return an error if this
	/// method is to be called again before `redeem_fulfillment` is
	/// called.
	fn process_redeem_orders(asset_id: Self::InvestmentId) -> Result<Self::Orders, Self::Error>;

	/// Signals the manager that the previously
	/// fetch invest orders for a given investment class
	/// will be fulfilled by fulfillment.
	fn invest_fulfillment(
		asset_id: Self::InvestmentId,
		fulfillment: Self::Fulfillment,
	) -> Result<(), Self::Error>;

	/// Signals the manager that the previously
	/// fetch redeem orders for a given investment class
	/// will be fulfilled by fulfillment.
	fn redeem_fulfillment(
		asset_id: Self::InvestmentId,
		fulfillment: Self::Fulfillment,
	) -> Result<(), Self::Error>;
}

/// A trait who's implementer provides means of accounting
/// for investments of a generic kind.
pub trait InvestmentAccountant<AccountId> {
	type Error;
	type InvestmentId;
	type InvestmentInfo;
	type Amount;

	/// Information about an asset. Must allow to derive
	/// owner, payment and denomination currency
	fn info(id: Self::InvestmentId) -> Result<Self::InvestmentInfo, Self::Error>;

	/// Return the balance of a given user for the given investmnet
	fn balance(id: Self::InvestmentId, who: &AccountId) -> Self::Amount;

	/// Transfer a given investment from source, to destination
	fn transfer(
		id: Self::InvestmentId,
		source: &AccountId,
		dest: &AccountId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Increases the existence of
	fn deposit(
		buyer: &AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Reduce the existence of an asset
	fn withdraw(
		seller: &AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;
}

/// Trait to handle investments in (presumably) foreign currencies, i.e., other
/// currencies than the pool currency.
///
/// NOTE: Has many similarities with the [Investment] trait.
pub trait ForeignInvestment<AccountId> {
	type Amount;
	type TrancheAmount;
	type CurrencyId;
	type Error: Debug;
	type InvestmentId;

	/// Initiates the increment of a foreign investment amount in
	/// `foreign_payment_currency` of who into the investment class
	/// `pool_currency` to amount.
	///
	/// NOTE: In general, we can assume that the foreign and pool currencies
	/// mismatch and that swapping one into the other happens asynchronously. In
	/// that case, the finalization of updating the investment needs to be
	/// handled decoupled from the ForeignInvestment trait, e.g., by some hook.
	fn increase_foreign_investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::Amount,
		foreign_payment_currency: Self::CurrencyId,
	) -> Result<(), Self::Error>;

	/// Initiates the decrement of a foreign investment amount in
	/// `foreign_payment_currency` of who into the investment class
	/// `pool_currency` to amount.
	///
	/// NOTE: In general, we can assume that the foreign and pool currencies
	/// mismatch and that swapping one into the other happens asynchronously. In
	/// that case, the finalization of updating the investment needs to be
	/// handled decoupled from the ForeignInvestment trait, e.g., by some hook.
	fn decrease_foreign_investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::Amount,
		foreign_payment_currency: Self::CurrencyId,
	) -> Result<(), Self::Error>;

	/// Initiates the increment of a foreign redemption amount for the given
	/// investment id.
	///
	/// NOTE: The `foreign_payout_currency` is only required to ensure
	/// subsequent redemption updating calls match to the original chosen
	/// `foreign_payment_currency`.
	fn increase_foreign_redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::TrancheAmount,
		foreign_payout_currency: Self::CurrencyId,
	) -> Result<(), Self::Error>;

	/// Initiates the decrement of a foreign redemption amount.
	///
	/// NOTES:
	/// * The decrementing redemption amount is bound by the previously
	///   incremented redemption amount.
	/// * The `foreign_payout_currency` is only required for the potential
	///   dispatch of a response message.
	fn decrease_foreign_redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::TrancheAmount,
		foreign_payout_currency: Self::CurrencyId,
	) -> Result<(), Self::Error>;

	/// Collect the results of a user's foreign invest orders for the given
	/// investment. If any amounts are not fulfilled they are directly
	/// appended to the next active order for this investment.
	fn collect_foreign_investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		foreign_currency: Self::CurrencyId,
	) -> Result<(), Self::Error>;

	/// Collect the results of a user's foreign redeem orders for the given
	/// investment. If any amounts are not fulfilled they are directly
	/// appended to the next active order for this investment.
	///
	/// NOTE: The currency of the collected amount will be `pool_currency`
	/// whereas the user eventually wants to receive it in
	/// `foreign_payout_currency`.
	fn collect_foreign_redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		foreign_payout_currency: Self::CurrencyId,
	) -> Result<(), Self::Error>;

	/// Returns, if possible, the currently unprocessed investment amount (in
	/// pool currency) of who into the given investment class.
	///
	/// NOTE: If the investment was (partially) processed, the unprocessed
	/// amount is only updated upon collecting.
	fn investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Amount, Self::Error>;

	/// Returns, if possible, the currently unprocessed redemption amount (in
	/// tranche tokens) of who into the given investment class.
	///
	/// NOTE: If the redemption was (partially) processed, the unprocessed
	/// amount is only updated upon collecting.
	fn redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::TrancheAmount, Self::Error>;
}
