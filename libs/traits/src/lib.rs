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

//! # A common trait lib for centrifuge
//!
//! This crate provides some common traits used by centrifuge.

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]

use cfg_primitives::Moment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::{Codec, DispatchResult, DispatchResultWithPostInfo},
	scale_info::TypeInfo,
	Parameter, RuntimeDebug,
};
use impl_trait_for_tuples::impl_for_tuples;
use sp_runtime::{
	traits::{
		AtLeast32BitUnsigned, Bounded, MaybeDisplay, MaybeMallocSizeOf, MaybeSerialize,
		MaybeSerializeDeserialize, Member, Zero,
	},
	DispatchError,
};
use sp_std::{fmt::Debug, hash::Hash, str::FromStr};

// #[cfg(test)]
pub mod mocks;

/// A trait used for loosely coupling the claim pallet with a reward mechanism.
///
/// ## Overview
/// The crowdloan reward mechanism is separated from the crowdloan claiming process, the latter
/// being generic, acting as a kind of proxy to the rewarding mechanism, that is specific to
/// to each crowdloan campaign. The aim of this pallet is to ensure that a claim for a reward
/// payout is well-formed, checking for replay attacks, spams or invalid claim (e.g. unknown
/// contributor, exceeding reward amount, ...).
/// See the [`crowdloan-reward`] pallet, that implements a reward mechanism with vesting, for
/// instance.
pub trait Reward {
	/// The account from the parachain, that the claimer provided in her/his transaction.
	type ParachainAccountId: Debug
		+ MaybeSerialize
		+ MaybeSerializeDeserialize
		+ Member
		+ Ord
		+ Parameter
		+ TypeInfo;

	/// The contribution amount in relay chain tokens.
	type ContributionAmount: AtLeast32BitUnsigned
		+ Codec
		+ Copy
		+ Debug
		+ Default
		+ MaybeSerializeDeserialize
		+ Member
		+ Parameter
		+ Zero
		+ TypeInfo;

	/// Block number type used by the runtime
	type BlockNumber: AtLeast32BitUnsigned
		+ Bounded
		+ Copy
		+ Debug
		+ Default
		+ FromStr
		+ Hash
		+ MaybeDisplay
		+ MaybeMallocSizeOf
		+ MaybeSerializeDeserialize
		+ Member
		+ Parameter
		+ TypeInfo;

	/// Rewarding function that is invoked from the claim pallet.
	///
	/// If this function returns successfully, any subsequent claim of the same claimer will be
	/// rejected by the claim module.
	fn reward(
		who: Self::ParachainAccountId,
		contribution: Self::ContributionAmount,
	) -> DispatchResultWithPostInfo;
}

/// A trait that can be used to fetch the nav and update nav for a given pool
pub trait PoolNAV<PoolId, Amount> {
	type ClassId;
	type Origin;
	// nav returns the nav and the last time it was calculated
	fn nav(pool_id: PoolId) -> Option<(Amount, u64)>;
	fn update_nav(pool_id: PoolId) -> Result<Amount, DispatchError>;
	fn initialise(origin: Self::Origin, pool_id: PoolId, class_id: Self::ClassId)
		-> DispatchResult;
}

/// A trait that support pool inspection operations such as pool existence checks and pool admin of permission set.
pub trait PoolInspect<AccountId, CurrencyId> {
	type PoolId: Parameter + Member + Debug + Copy + Default + TypeInfo;
	type TrancheId: Parameter + Member + Debug + Copy + Default + TypeInfo;
	type Rate;
	type Moment;

	/// check if the pool exists
	fn pool_exists(pool_id: Self::PoolId) -> bool;
	fn tranche_exists(pool_id: Self::PoolId, tranche_id: Self::TrancheId) -> bool;
	fn get_tranche_token_price(
		pool_id: Self::PoolId,
		tranche_id: Self::TrancheId,
	) -> Option<PriceValue<CurrencyId, Self::Rate, Self::Moment>>;
}

/// A trait that support pool reserve operations such as withdraw and deposit
pub trait PoolReserve<AccountId, CurrencyId>: PoolInspect<AccountId, CurrencyId> {
	type Balance;

	/// Withdraw `amount` from the reserve to the `to` account.
	fn withdraw(pool_id: Self::PoolId, to: AccountId, amount: Self::Balance) -> DispatchResult;

	/// Deposit `amount` from the `from` account into the reserve.
	fn deposit(pool_id: Self::PoolId, from: AccountId, amount: Self::Balance) -> DispatchResult;
}

/// A trait that can be used to retrieve the current price for a currency
pub struct CurrencyPair<CurrencyId> {
	pub base: CurrencyId,
	pub quote: CurrencyId,
}

pub struct PriceValue<CurrencyId, Rate, Moment> {
	pub pair: CurrencyPair<CurrencyId>,
	pub price: Rate,
	pub last_updated: Moment,
}

pub trait CurrencyPrice<CurrencyId> {
	type Rate;
	type Moment;

	/// Retrieve the latest price of `base` currency, denominated in the `quote` currency
	/// If `quote` currency is not passed, then the default `quote` currency is used (when possible)
	fn get_latest(
		base: CurrencyId,
		quote: Option<CurrencyId>,
	) -> Option<PriceValue<CurrencyId, Self::Rate, Self::Moment>>;
}

/// A trait that can be used to calculate interest accrual for debt
pub trait InterestAccrual<InterestRate, Balance, Adjustment> {
	type NormalizedDebt: Member + Parameter + MaxEncodedLen + TypeInfo + Copy + Zero;

	/// Calculate the current debt using normalized debt * cumulative rate
	fn current_debt(
		interest_rate_per_sec: InterestRate,
		normalized_debt: Self::NormalizedDebt,
	) -> Result<Balance, DispatchError>;

	/// Calculate a previous debt using normalized debt * previous cumulative rate
	///
	/// If `when` is further in the past than the last time the
	/// normalized debt was adjusted, this will return nonsense
	/// (effectively "rewinding the clock" to before the value was
	/// valid)
	fn previous_debt(
		interest_rate_per_sec: InterestRate,
		normalized_debt: Self::NormalizedDebt,
		when: Moment,
	) -> Result<Balance, DispatchError>;

	/// Increase or decrease the normalized debt
	fn adjust_normalized_debt(
		interest_rate_per_sec: InterestRate,
		normalized_debt: Self::NormalizedDebt,
		adjustment: Adjustment,
	) -> Result<Self::NormalizedDebt, DispatchError>;

	/// Re-normalize a debt for a new interest rate
	fn renormalize_debt(
		old_interest_rate: InterestRate,
		new_interest_rate: InterestRate,
		normalized_debt: Self::NormalizedDebt,
	) -> Result<Self::NormalizedDebt, DispatchError>;

	/// Indicate that a yearly rate is in use
	///
	/// Validates that the rate is allowed, and converts it to a per-second rate for future operations
	fn reference_yearly_rate(
		interest_rate_per_year: InterestRate,
	) -> Result<InterestRate, DispatchError>;

	/// Indicate that a rate is in use
	fn reference_rate(interest_rate_per_sec: InterestRate) -> DispatchResult;

	/// Indicate that a rate is no longer in use
	fn unreference_rate(interest_rate_per_sec: InterestRate) -> DispatchResult;

	/// Verifies a yearly additive rate and converts it to a per-second additive rate
	fn convert_additive_rate_to_per_sec(
		interset_rate_per_year: InterestRate,
	) -> Result<InterestRate, DispatchError>;
}

pub trait Permissions<AccountId> {
	type Scope;
	type Role;
	type Error: Debug;
	type Ok: Debug;

	fn has(scope: Self::Scope, who: AccountId, role: Self::Role) -> bool;

	fn add(scope: Self::Scope, who: AccountId, role: Self::Role) -> Result<Self::Ok, Self::Error>;

	fn remove(
		scope: Self::Scope,
		who: AccountId,
		role: Self::Role,
	) -> Result<Self::Ok, Self::Error>;
}

pub trait Properties {
	type Property;
	type Error;
	type Ok;

	fn exists(&self, property: Self::Property) -> bool;

	fn empty(&self) -> bool;

	fn rm(&mut self, property: Self::Property) -> Result<Self::Ok, Self::Error>;

	fn add(&mut self, property: Self::Property) -> Result<Self::Ok, Self::Error>;
}

pub trait PoolUpdateGuard {
	type PoolDetails;
	type ScheduledUpdateDetails;
	type Moment: Copy;

	fn released(
		pool: &Self::PoolDetails,
		update: &Self::ScheduledUpdateDetails,
		now: Self::Moment,
	) -> bool;
}

pub trait PreConditions<T> {
	type Result;

	fn check(t: T) -> Self::Result;
}

#[impl_for_tuples(1, 10)]
#[tuple_types_custom_trait_bound(PreConditions<T, Result = bool>)]
#[allow(clippy::redundant_clone)]
impl<T> PreConditions<T> for Tuple
where
	T: Clone,
{
	type Result = bool;

	fn check(t: T) -> Self::Result {
		for_tuples!( #( <Tuple as PreConditions::<T>>::check(t.clone()) )&* )
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Always;
impl<T> PreConditions<T> for Always {
	type Result = bool;

	fn check(_t: T) -> bool {
		true
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Never;
impl<T> PreConditions<T> for Never {
	type Result = bool;

	fn check(_t: T) -> bool {
		false
	}
}

/// Trait for converting a pool+tranche ID pair to a CurrencyId
///
/// This should be implemented in the runtime to convert from the
/// PoolId and TrancheId types to a CurrencyId that represents that
/// tranche.
///
/// The pool epoch logic assumes that every tranche has a UNIQUE
/// currency, but nothing enforces that. Failure to ensure currency
/// uniqueness will almost certainly cause some wild bugs.
pub trait TrancheToken<PoolId, TrancheId, CurrencyId> {
	fn tranche_token(pool: PoolId, tranche: TrancheId) -> CurrencyId;
}

/// A trait, when implemented allows to invest into
/// investment classes
pub trait Investment<AccountId> {
	type Error;
	type InvestmentId;
	type Amount;

	/// Updates the current investment amount of who into the
	/// investment class to amount.
	/// Meaning: if amount < previous investment, then investment
	/// will be reduced, and increases in the opposite case.
	fn update_investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Returns, if possible, the current investment amount of who into the given investment
	/// class
	fn investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Amount, Self::Error>;

	/// Updates the current redemption amount of who into the
	/// investment class to amount.
	/// Meaning: if amount < previous redemption, then redemption
	/// will be reduced, and increases in the opposite case.
	fn update_redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Returns, if possible, the current redemption amount of who into the given investment
	/// class
	fn redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Amount, Self::Error>;
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
	fn invest_orders(asset_id: Self::InvestmentId) -> Result<Self::Orders, Self::Error>;

	/// When called the manager return the current
	/// redeem orders for the given investment class.
	fn redeem_orders(asset_id: Self::InvestmentId) -> Result<Self::Orders, Self::Error>;

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
	type InvestmentInfo: InvestmentProperties<AccountId, Id = Self::InvestmentId>;
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

	/// Increases the existance of
	fn deposit(
		buyer: &AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Reduce the existance of an asset
	fn withdraw(
		seller: &AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;
}

/// A trait that allows to retrieve information
/// about an investment class.
pub trait InvestmentProperties<AccountId> {
	/// The overarching Currency that payments
	/// for this class are made in
	type Currency;
	/// Who the investment class can be identified
	type Id;

	/// Returns the owner of the investment class
	fn owner(&self) -> AccountId;

	/// Returns the id of the investment class
	fn id(&self) -> Self::Id;

	/// Returns the currency in which the investment class
	/// can be bought.
	fn payment_currency(&self) -> Self::Currency;

	/// Returns the account a payment for the investment class
	/// must be made to.
	///
	/// Defaults to owner.
	fn payment_account(&self) -> AccountId {
		self.owner()
	}
}

impl<AccountId, T: InvestmentProperties<AccountId>> InvestmentProperties<AccountId> for &T {
	type Currency = T::Currency;
	type Id = T::Id;

	fn owner(&self) -> AccountId {
		(*self).owner()
	}

	fn id(&self) -> Self::Id {
		(*self).id()
	}

	fn payment_currency(&self) -> Self::Currency {
		(*self).payment_currency()
	}

	fn payment_account(&self) -> AccountId {
		(*self).payment_account()
	}
}

pub mod fees {
	use codec::FullCodec;
	use frame_support::{dispatch::DispatchResult, traits::tokens::Balance};
	use scale_info::TypeInfo;
	use sp_runtime::traits::MaybeSerializeDeserialize;

	/// Type used for identity the key used to retrieve the fees.
	pub trait FeeKey:
		FullCodec + TypeInfo + MaybeSerializeDeserialize + sp_std::fmt::Debug + Clone + PartialEq
	{
	}

	impl<
			T: FullCodec
				+ TypeInfo
				+ MaybeSerializeDeserialize
				+ sp_std::fmt::Debug
				+ Clone
				+ PartialEq,
		> FeeKey for T
	{
	}

	/// A way to identify a fee value.
	pub enum Fee<Balance, FeeKey> {
		/// The fee value itself.
		Balance(Balance),

		/// The fee value is already stored and identified by a key.
		Key(FeeKey),
	}

	/// A trait that used to deal with fees
	pub trait Fees {
		type AccountId;
		type Balance: Balance;
		type FeeKey: FeeKey;

		/// Get the fee balance for a fee key
		fn fee_value(key: Self::FeeKey) -> Self::Balance;

		/// Pay an amount of fee to the block author
		/// If the `from` account has not enough balance or the author is invalid the fees are not
		/// paid.
		fn fee_to_author(
			from: &Self::AccountId,
			fee: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult;

		/// Burn an amount of fee
		/// If the `from` account has not enough balance the fees are not paid.
		fn fee_to_burn(
			from: &Self::AccountId,
			fee: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult;

		/// Send an amount of fee to the treasury
		/// If the `from` account has not enough balance the fees are not paid.
		fn fee_to_treasury(
			from: &Self::AccountId,
			fee: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult;
	}

	pub struct NoFees<AccountId, Balance>(sp_std::marker::PhantomData<(AccountId, Balance)>);

	impl<A, B: Balance> Fees for NoFees<A, B> {
		type AccountId = A;
		type Balance = B;
		type FeeKey = ();

		fn fee_value(_: Self::FeeKey) -> Self::Balance {
			Self::Balance::default()
		}

		fn fee_to_author(
			_: &Self::AccountId,
			_: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult {
			Ok(())
		}

		fn fee_to_burn(_: &Self::AccountId, _: Fee<Self::Balance, Self::FeeKey>) -> DispatchResult {
			Ok(())
		}

		fn fee_to_treasury(
			_: &Self::AccountId,
			_: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult {
			Ok(())
		}
	}

	#[cfg(feature = "std")]
	pub mod test_util {
		use std::{cell::RefCell, thread::LocalKey};

		use frame_support::{
			dispatch::DispatchResult,
			traits::{tokens::Balance, Get},
		};

		use super::{Fee, FeeKey, Fees};

		pub struct FeeState<Author, Balance> {
			pub author: Author,
			pub balance: Balance,
		}

		pub struct FeesState<Author, Balance, KeyFee> {
			pub author_fees: Vec<FeeState<Author, Balance>>,
			pub burn_fees: Vec<FeeState<Author, Balance>>,
			pub treasury_fees: Vec<FeeState<Author, Balance>>,
			pub initializer: Box<dyn Fn(KeyFee) -> Balance + 'static>,
		}

		impl<A, B, K> FeesState<A, B, K> {
			pub fn no_fees(&self) -> bool {
				self.author_fees.is_empty()
					&& self.burn_fees.is_empty()
					&& self.treasury_fees.is_empty()
			}
		}

		impl<A, B, K> FeesState<A, B, K> {
			pub fn new(initializer: impl Fn(K) -> B + 'static) -> Self {
				Self {
					author_fees: Default::default(),
					burn_fees: Default::default(),
					treasury_fees: Default::default(),
					initializer: Box::new(initializer),
				}
			}
		}

		#[macro_export]
		macro_rules! impl_mock_fees_state {
			($name:ident, $account:ty, $balance:ty, $feekey:ty, $initializer:expr) => {
				use std::{cell::RefCell, thread::LocalKey};

				use cfg_traits::fees::test_util::FeesState;

				thread_local! {
					pub static STATE: RefCell<
						FeesState<$account, $balance, $feekey>,
					> = RefCell::new(FeesState::new($initializer));
				}

				parameter_types! {
					pub $name: &'static LocalKey<
						RefCell<FeesState<$account, $balance, $feekey>>
					> = &STATE;
				}
			};
		}

		pub struct MockFees<AccountId, Balance, FeeKey, State>(
			sp_std::marker::PhantomData<(AccountId, Balance, FeeKey, State)>,
		);

		impl<
				A: Clone + 'static,
				B: Balance + 'static,
				K: FeeKey + 'static,
				S: Get<&'static LocalKey<RefCell<FeesState<A, B, K>>>>,
			> Fees for MockFees<A, B, K, S>
		{
			type AccountId = A;
			type Balance = B;
			type FeeKey = K;

			fn fee_value(key: Self::FeeKey) -> Self::Balance {
				S::get().with(|state| (state.borrow().initializer)(key))
			}

			fn fee_to_author(
				author: &Self::AccountId,
				fee: Fee<Self::Balance, Self::FeeKey>,
			) -> DispatchResult {
				let balance = Self::balance(fee);
				S::get().with(|state| {
					state.borrow_mut().author_fees.push(FeeState {
						author: author.clone(),
						balance,
					});
				});
				Ok(())
			}

			fn fee_to_burn(
				author: &Self::AccountId,
				fee: Fee<Self::Balance, Self::FeeKey>,
			) -> DispatchResult {
				let balance = Self::balance(fee);
				S::get().with(|state| {
					state.borrow_mut().burn_fees.push(FeeState {
						author: author.clone(),
						balance,
					});
				});
				Ok(())
			}

			fn fee_to_treasury(
				author: &Self::AccountId,
				fee: Fee<Self::Balance, Self::FeeKey>,
			) -> DispatchResult {
				let balance = Self::balance(fee);
				S::get().with(|state| {
					state.borrow_mut().treasury_fees.push(FeeState {
						author: author.clone(),
						balance,
					});
				});
				Ok(())
			}
		}

		impl<
				A: Clone + 'static,
				B: Balance + 'static,
				K: FeeKey + 'static,
				S: Get<&'static LocalKey<RefCell<FeesState<A, B, K>>>>,
			> MockFees<A, B, K, S>
		{
			fn balance(fee: Fee<B, K>) -> B {
				match fee {
					Fee::Balance(balance) => balance,
					Fee::Key(key) => Self::fee_value(key),
				}
			}
		}
	}
}

pub mod ops {
	use sp_runtime::{
		traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub},
		ArithmeticError,
	};

	pub trait Signum {
		fn signum(&self) -> i8;
	}

	macro_rules! signum_variant_impl {
		($t:ty) => {
			impl Signum for $t {
				fn signum(&self) -> i8 {
					(*self as $t).signum() as i8
				}
			}
		};
	}

	macro_rules! signum_pos_impl {
		($t:ty) => {
			impl Signum for $t {
				fn signum(&self) -> i8 {
					1
				}
			}
		};
	}

	signum_pos_impl!(u8);
	signum_pos_impl!(u16);
	signum_pos_impl!(u32);
	signum_pos_impl!(u64);
	signum_pos_impl!(u128);
	signum_variant_impl!(i8);
	signum_variant_impl!(i16);
	signum_variant_impl!(i32);
	signum_variant_impl!(i64);
	signum_variant_impl!(i128);
	signum_variant_impl!(f32);
	signum_variant_impl!(f64);

	/// Performs addition that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureAdd: CheckedAdd + Signum {
		/// Adds two numbers, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::EnsureAdd;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     u32::MAX.ensure_add(&1)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     i32::MIN.ensure_add(&-1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_add(&self, v: &Self) -> Result<Self, ArithmeticError> {
			self.checked_add(v).ok_or_else(|| match v.signum() {
				-1 => ArithmeticError::Underflow,
				_ => ArithmeticError::Overflow,
			})
		}
	}

	/// Performs subtraction that returns `ArithmeticError` instead of wrapping around on underflow.
	pub trait EnsureSub: CheckedSub + Signum {
		/// Subtracts two numbers, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::EnsureSub;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     0u32.ensure_sub(&1)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     i32::MAX.ensure_sub(&-1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// ```
		fn ensure_sub(&self, v: &Self) -> Result<Self, ArithmeticError> {
			self.checked_sub(v).ok_or_else(|| match v.signum() {
				-1 => ArithmeticError::Overflow,
				_ => ArithmeticError::Underflow,
			})
		}
	}

	/// Performs multiplication that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureMul: CheckedMul + Signum {
		/// Multiplies two numbers, checking for overflow. If overflow happens,
		/// `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::EnsureMul;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     u32::MAX.ensure_mul(&2)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     i32::MAX.ensure_mul(&-2)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_mul(&self, v: &Self) -> Result<Self, ArithmeticError> {
			self.checked_mul(v)
				.ok_or_else(|| match self.signum() * v.signum() {
					-1 => ArithmeticError::Underflow,
					_ => ArithmeticError::Overflow,
				})
		}
	}

	/// Performs division that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureDiv: CheckedDiv {
		/// Divides two numbers, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::EnsureDiv;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic() -> DispatchResult {
		///     1.ensure_div(&0)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic(), Err(ArithmeticError::DivisionByZero.into()));
		/// ```
		fn ensure_div(&self, v: &Self) -> Result<Self, ArithmeticError> {
			self.checked_div(v).ok_or(ArithmeticError::DivisionByZero)
		}
	}

	impl<T: CheckedAdd + Signum> EnsureAdd for T {}
	impl<T: CheckedSub + Signum> EnsureSub for T {}
	impl<T: CheckedMul + Signum> EnsureMul for T {}
	impl<T: CheckedDiv> EnsureDiv for T {}

	/// Performs self addition that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureAddAssign: EnsureAdd {
		/// Adds two numbers overwriting the left hand one, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::EnsureAddAssign;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     let mut max = u32::MAX;
		///     max.ensure_add_assign(&1)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     let mut max = i32::MIN;
		///     max.ensure_add_assign(&-1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_add_assign(&mut self, v: &Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_add(v)?;
			Ok(())
		}
	}

	/// Performs self subtraction that returns `ArithmeticError` instead of wrapping around on underflow.
	pub trait EnsureSubAssign: EnsureSub {
		/// Subtracts two numbers overwriting the left hand one, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::EnsureSubAssign;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     let mut zero: u32 = 0;
		///     zero.ensure_sub_assign(&1)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     let mut zero = i32::MAX;
		///     zero.ensure_sub_assign(&-1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// ```
		fn ensure_sub_assign(&mut self, v: &Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_sub(v)?;
			Ok(())
		}
	}

	/// Performs self multiplication that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureMulAssign: EnsureMul {
		/// Multiplies two numbers overwriting the left hand one, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::EnsureMulAssign;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     let mut max = u32::MAX;
		///     max.ensure_mul_assign(&2)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     let mut max = i32::MAX;
		///     max.ensure_mul_assign(&-2)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_mul_assign(&mut self, v: &Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_mul(v)?;
			Ok(())
		}
	}

	/// Performs self division that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureDivAssign: EnsureDiv {
		/// Divides two numbers overwriting the left hand one, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::EnsureDivAssign;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic() -> DispatchResult {
		///     let mut one = 1;
		///     one.ensure_div_assign(&0)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic(), Err(DispatchError::Arithmetic(ArithmeticError::DivisionByZero)));
		/// ```
		fn ensure_div_assign(&mut self, v: &Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_div(v)?;
			Ok(())
		}
	}

	impl<T: EnsureAdd> EnsureAddAssign for T {}
	impl<T: EnsureSub> EnsureSubAssign for T {}
	impl<T: EnsureMul> EnsureMulAssign for T {}
	impl<T: EnsureDiv> EnsureDivAssign for T {}
}
