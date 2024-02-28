use frame_support::{scale_info::TypeInfo, RuntimeDebug};
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

impl<Amount, Currency: PartialEq> Swap<Amount, Currency> {
	pub fn has_same_currencies(&self) -> bool {
		self.currency_in == self.currency_out
	}

	pub fn is_same_direction(&self, other: &Self) -> Result<bool, DispatchError> {
		if self.currency_in == other.currency_in && self.currency_out == other.currency_out {
			Ok(true)
		} else if self.currency_in == other.currency_out && self.currency_out == other.currency_in {
			Ok(false)
		} else {
			Err(DispatchError::Other("Swap contains different currencies"))
		}
	}
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

	/// Fill an existing order up to the provided amount.
	///  * If `amount` equals the `order.amount_out`, the order is completely
	///    fulfilled.
	///  * Else, the order is partially fulfilled for `amount /
	///    order.amount_out`%.
	fn fill_order(
		account: Account,
		order_id: Self::OrderId,
		amount: Self::BalanceOut,
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
}

/// A representation of a currency swap in process.
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct SwapState<AmountIn, AmountOut, Currency> {
	/// Swap not yet processed with the pending outcomming amount
	pub remaining: Swap<AmountOut, Currency>,

	/// Amount of incoming currency already swapped
	pub swapped_in: AmountIn,

	/// Amount of incoming currency already swapped denominated in outgoing
	/// currency
	pub swapped_out: AmountOut,
}

/// Used as result of `Pallet::apply_swap()`
/// Amounts are donominated referenced by the `new_swap` paramenter given to
/// `apply_swap()`
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SwapStatus<Amount> {
	/// The incoming amount already swapped and available to use.
	pub swapped: Amount,

	/// The outgoing amount pending to be swapped
	pub pending: Amount,
}

/// Trait to perform swaps without handling directly an order book
pub trait Swaps<AccountId> {
	type Amount;
	type CurrencyId;
	type SwapId;

	/// Apply a swap over a current possible swap state.
	/// - If there was no previous swap, it adds it.
	/// - If there was a swap in the same direction, it increments it.
	/// - If there was a swap in the opposite direction:
	///   - If the amount is smaller, it decrements it.
	///   - If the amount is the same, it removes the inverse swap.
	///   - If the amount is greater, it removes the inverse swap and create
	///     another with the excess
	///
	/// The returned status contains the swapped amount after this call
	/// (denominated in the incoming currency) and the pending amounts to be
	/// swapped.
	fn apply_swap(
		who: &AccountId,
		swap_id: Self::SwapId,
		swap: Swap<Self::Amount, Self::CurrencyId>,
	) -> Result<SwapStatus<Self::Amount>, DispatchError>;

	/// Cancel a swap partially or completely. The amount should be expressed in
	/// the same currency as the the currency_out of the pending amount.
	/// - If there was no previous swap, it errors outs.
	/// - If there was a swap with other currency out, it errors outs.
	/// - If there was a swap with same currency out:
	///   - If the amount is smaller, it decrements it.
	///   - If the amount is the same, it removes the inverse swap.
	///   - If the amount is greater, it errors out
	fn cancel_swap(
		who: &AccountId,
		swap_id: Self::SwapId,
		amount: Self::Amount,
		currency_id: Self::CurrencyId,
	) -> DispatchResult;

	/// Returns the pending amount for a pending swap. The direction of the swap
	/// is determined by the `from_currency` parameter. The amount returned is
	/// denominated in the same currency as the given `from_currency`.
	fn pending_amount(
		who: &AccountId,
		swap_id: Self::SwapId,
		from_currency: Self::CurrencyId,
	) -> Result<Self::Amount, DispatchError>;
}
