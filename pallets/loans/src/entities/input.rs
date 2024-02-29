use frame_support::{storage::bounded_btree_map::BoundedBTreeMap, RuntimeDebugNoBound};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{ArithmeticError, DispatchError};

use crate::{
	entities::pricing::external::ExternalAmount,
	pallet::{Config, Error},
	types::RepaidAmount,
};

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum PrincipalInput<T: Config> {
	Internal(T::Balance),
	External(ExternalAmount<T>),
}

impl<T: Config> PrincipalInput<T> {
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

	pub fn external(&self) -> Result<ExternalAmount<T>, DispatchError> {
		match self {
			Self::Internal(_) => Err(Error::<T>::MismatchedPricingMethod.into()),
			Self::External(principal) => Ok(principal.clone()),
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RepaidInput<T: Config> {
	pub principal: PrincipalInput<T>,
	pub interest: T::Balance,
	pub unscheduled: T::Balance,
}

impl<T: Config> RepaidInput<T> {
	pub fn repaid_amount(&self) -> Result<RepaidAmount<T::Balance>, ArithmeticError> {
		Ok(RepaidAmount {
			principal: self.principal.balance()?,
			interest: self.interest,
			unscheduled: self.unscheduled,
		})
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum PriceCollectionInput<T: Config> {
	Empty,
	Custom(BoundedBTreeMap<T::PriceId, T::Balance, T::MaxActiveLoansPerPool>),
	FromRegistry,
}
