use frame_support::pallet_prelude::{RuntimeDebug, TypeInfo};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::fmt::Debug;

/// Determines an order price
#[derive(Clone, Copy, Debug, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
pub enum OrderRatio<Ratio> {
	Market,
	Custom(Ratio),
}

/// A simple representation of a currency swap.
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct Swap<Amount, Currency> {
	/// The incoming currency, i.e. the desired one.
	pub currency_in: Currency,

	/// The outgoing currency, i.e. the one which should be replaced.
	pub currency_out: Currency,

	/// The amount of outcoming currency that will be swapped.
	pub amount_out: Amount,
}

/// The information of a swap order
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct OrderInfo<Balance, Currency, Ratio> {
	/// The underlying currency swap
	pub swap: Swap<Balance, Currency>,
	/// The ratio at which the swap should happen
	pub ratio: OrderRatio<Ratio>,
}

pub trait TokenSwaps<Account> {
	type CurrencyId;
	type BalanceOut;
	type BalanceIn;
	type Ratio;
	type OrderId;

	/// Swap tokens selling `amount_out` of `currency_out` and buying
	/// `currency_in` given an order ratio.
	fn place_order(
		account: Account,
		currency_in: Self::CurrencyId,
		currency_out: Self::CurrencyId,
		amount_out: Self::BalanceOut,
		ratio: OrderRatio<Self::Ratio>,
	) -> Result<Self::OrderId, DispatchError>;

	/// Update an existing active order.
	fn update_order(
		order_id: Self::OrderId,
		amount_out: Self::BalanceOut,
		ratio: OrderRatio<Self::Ratio>,
	) -> DispatchResult;

	/// Fill an existing order up to the provided `buy_amount`.
	///  * If `buy_amount` equals the `order.amount_out`, the order is
	///    completely fulfilled.
	///  * Else, the order is partially fulfilled for `amount /
	///    order.amount_out`%.
	///
	/// NOTE:
	/// * The `buy_amount` is outgoing currency amount of the order.
	/// * The `max_sell_amount` protects `account` from extreme market
	/// conditions or being front-run. It refers to the incoming currency amount
	/// of the order.
	fn fill_order(
		account: Account,
		order_id: Self::OrderId,
		buy_amount: Self::BalanceOut,
		max_sell_amount: Self::BalanceIn,
	) -> DispatchResult;

	/// Cancel an already active order.
	fn cancel_order(order: Self::OrderId) -> DispatchResult;

	/// Retrieve the details of the order if it exists.
	fn get_order_details(
		order: Self::OrderId,
	) -> Option<OrderInfo<Self::BalanceOut, Self::CurrencyId, Self::Ratio>>;

	/// Makes a conversion between 2 currencies using the market ratio between
	/// them.
	fn convert_by_market(
		currency_in: Self::CurrencyId,
		currency_out: Self::CurrencyId,
		amount_out: Self::BalanceOut,
	) -> Result<Self::BalanceIn, DispatchError>;

	/// Returns the conversion ratio to convert currency out into currency in,
	fn market_ratio(
		currency_in: Self::CurrencyId,
		currency_out: Self::CurrencyId,
	) -> Result<Self::Ratio, DispatchError>;
}

/// A representation of a currency swap in process.
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct SwapInfo<AmountIn, AmountOut, Currency, Ratio> {
	/// Swap not yet processed with the pending outcomming amount
	pub remaining: Swap<AmountOut, Currency>,

	/// Amount of incoming currency already swapped
	pub swapped_in: AmountIn,

	/// Amount of incoming currency already swapped denominated in outgoing
	/// currency
	pub swapped_out: AmountOut,

	/// Ratio used to swap `swapped_out` into `swapped_in`
	pub ratio: Ratio,
}
