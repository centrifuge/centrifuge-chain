// Copyright 2023 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::{AccountId, LoanId};
use cfg_types::{fixed_point::Rate, pools::FeeBucket};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{dispatch::TypeInfo, BoundedVec, RuntimeDebug};

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub enum FeeAmount<Balance, Rate> {
	ShareOfPortfolioValuation(Rate),
	// Future options: AmountPerYear, AmountPerMonth, ...
	// TODO: AmountPerSecond(Balance) might be sufficient
	AmountPerYear(Balance),
	AmountPerMonth(Balance),
}

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub enum FeeAmountType<Balance, Rate> {
	/// A fixed fee is deducted automatically every epoch
	Fixed { amount: FeeAmount<Balance, Rate> },

	/// A fee can be charged up to a limit, paid every epoch
	ChargedUpTo { limit: FeeAmount<Balance, Rate> },
}

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub enum FeeEditor<AccountId> {
	Root,
	Account(AccountId),
}

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub struct PoolFee<AccountId, Balance, Rate> {
	/// Account that the fees are sent to
	pub destination: AccountId,

	/// Account that can update this fee
	pub editor: FeeEditor<AccountId>,

	/// Amount of fees that can be charged
	pub amount: FeeAmountType<Balance, Rate>,
}

/// Represents pool changes which might require to complete further guarding
/// checks.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub enum Change<AccountId, Balance, Rate> {
	AppendFee(FeeBucket, PoolFee<AccountId, Balance, Rate>),
	RemoveFee(FeeBucket, PoolFee<AccountId, Balance, Rate>),
}

// NOTE: Remark feature will be a separate feature in the future (post Pool Fees
// MVP). The following enum is necessary to deliver the MVP.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]

pub enum Remark<Hash, LoanId, Meta> {
	Loan { id: LoanId, meta: Meta },
	IpfsHash(Hash),
	Metadata(Meta),
}

pub type IpfsHash = [u8; 46];
