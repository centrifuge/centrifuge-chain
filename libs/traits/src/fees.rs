use frame_support::{dispatch::DispatchResult, traits::tokens::Balance};

/// A way to identify a fee value.
pub enum Fee<Balance, FeeKey> {
	/// The fee value itself.
	Balance(Balance),

	/// The fee value is already stored and identified by a key.
	Key(FeeKey),
}

impl<Balance: Copy, FeeKey: Clone> Fee<Balance, FeeKey> {
	pub fn value<F: Fees<Balance = Balance, FeeKey = FeeKey>>(&self) -> Balance {
		match self {
			Fee::Balance(value) => *value,
			Fee::Key(key) => F::fee_value(key.clone()),
		}
	}
}

/// A trait that used to deal with fees
pub trait Fees {
	type AccountId;
	type Balance: Balance;
	type FeeKey;

	/// Get the fee balance for a fee key
	fn fee_value(key: Self::FeeKey) -> Self::Balance;

	/// Pay an amount of fee to the block author
	/// If the `from` account has not enough balance or the author is
	/// invalid the fees are not paid.
	fn fee_to_author(
		from: &Self::AccountId,
		fee: Fee<Self::Balance, Self::FeeKey>,
	) -> DispatchResult;

	/// Burn an amount of fee
	/// If the `from` account has not enough balance the fees are not paid.
	fn fee_to_burn(from: &Self::AccountId, fee: Fee<Self::Balance, Self::FeeKey>)
		-> DispatchResult;

	/// Send an amount of fee to the treasury
	/// If the `from` account has not enough balance the fees are not paid.
	fn fee_to_treasury(
		from: &Self::AccountId,
		fee: Fee<Self::Balance, Self::FeeKey>,
	) -> DispatchResult;

	/// Allows to initialize an initial state required for a pallet that pay a
	/// fee
	#[cfg(feature = "runtime-benchmarks")]
	fn add_fee_requirements(_from: &Self::AccountId, _fee: Fee<Self::Balance, Self::FeeKey>) {}
}

/// Trait to pay fees
/// This trait can be used by a pallet to just pay fees without worring
/// about the value or where the fee goes.
pub trait PayFee<AccountId> {
	/// Pay the fee using a payer
	fn pay(payer: &AccountId) -> DispatchResult;

	/// Allows to initialize an initial state required for a pallet that
	/// calls `pay()`.
	#[cfg(feature = "runtime-benchmarks")]
	fn add_pay_requirements(_payer: &AccountId) {}
}

/// Type to avoid paying fees
pub struct NoPayFee;
impl<AccountId> PayFee<AccountId> for NoPayFee {
	fn pay(_: &AccountId) -> DispatchResult {
		Ok(())
	}
}
