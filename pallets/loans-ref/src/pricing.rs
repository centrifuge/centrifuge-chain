use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{RuntimeDebug, RuntimeDebugNoBound};
use scale_info::TypeInfo;

use crate::pallet::Config;

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
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum ActivePricing<T: Config> {
	/// External attributes
	Internal(internal::InternalActivePricing<T>),

	/// Internal attributes
	External(external::ExternalActivePricing<T>),
}
