use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebugNoBound;
use scale_info::TypeInfo;
use sp_runtime::{ArithmeticError, DispatchError};

use crate::{
	pallet::{Config, Error},
	types::RepaidAmount,
};

pub mod external;
pub mod internal;

/// Loan pricing method
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(bound = ""))]
pub enum Pricing<T: Config> {
	/// Calculated internally
	Internal(internal::InternalPricing<T>),

	/// Calculated externally
	External(external::ExternalPricing<T>),
}

/// Pricing attributes for active loans
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(bound = ""))]
pub enum ActivePricing<T: Config> {
	/// External attributes
	Internal(internal::InternalActivePricing<T>),

	/// Internal attributes
	External(external::ExternalActivePricing<T>),
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum PricingAmount<T: Config> {
	Internal(T::Balance),
	External(external::ExternalAmount<T>),
}

impl<T: Config> PricingAmount<T> {
	pub fn balance(&self) -> Result<T::Balance, ArithmeticError> {
		match self {
			Self::Internal(amount) => Ok(*amount),
			Self::External(external) => external.balance(),
		}
	}

	pub fn internal(&self) -> Result<T::Balance, DispatchError> {
		match self {
			Self::Internal(amount) => Ok(*amount),
			Self::External(_) => Err(Error::<T>::MismatchedPricingMethod.into()),
		}
	}

	pub fn external(&self) -> Result<external::ExternalAmount<T>, DispatchError> {
		match self {
			Self::Internal(_) => Err(Error::<T>::MismatchedPricingMethod.into()),
			Self::External(principal) => Ok(principal.clone()),
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RepaidPricingAmount<T: Config> {
	pub principal: PricingAmount<T>,
	pub interest: T::Balance,
	pub unscheduled: T::Balance,
}

impl<T: Config> RepaidPricingAmount<T> {
	pub fn repaid_amount(&self) -> Result<RepaidAmount<T::Balance>, ArithmeticError> {
		Ok(RepaidAmount {
			principal: self.principal.balance()?,
			interest: self.interest,
			unscheduled: self.unscheduled,
		})
	}
}
