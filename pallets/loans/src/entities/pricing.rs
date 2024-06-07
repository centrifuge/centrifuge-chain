use frame_support::RuntimeDebugNoBound;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use crate::{entities::interest::ActiveInterestRate, pallet::Config};

pub mod external;
pub mod internal;

/// Loan pricing method
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum Pricing<T: Config> {
	/// Calculated internally
	Internal(internal::InternalPricing<T>),

	/// Calculated externally
	External(external::ExternalPricing<T>),
}

/// Pricing attributes for active loans
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum ActivePricing<T: Config> {
	/// External attributes
	Internal(internal::InternalActivePricing<T>),

	/// Internal attributes
	External(external::ExternalActivePricing<T>),
}

impl<T: Config> ActivePricing<T> {
	pub fn interest(&self) -> &ActiveInterestRate<T> {
		match self {
			Self::Internal(inner) => &inner.interest,
			Self::External(inner) => &inner.interest,
		}
	}

	pub fn interest_mut(&mut self) -> &mut ActiveInterestRate<T> {
		match self {
			Self::Internal(inner) => &mut inner.interest,
			Self::External(inner) => &mut inner.interest,
		}
	}
}
