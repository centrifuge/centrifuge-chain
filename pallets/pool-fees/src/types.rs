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
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::TypeInfo;

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
enum FeeRate<Rate> {
	ShareOfPortfolioValuation(Rate),
	// Future options: AmountPerYear, AmountPerMonth, ...
}

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]

enum FeeAmount<Rate> {
	/// A fixed fee is deducted automatically every epoch
	Fixed { amount: FeeRate<Rate> },

	/// A fee can be charged up to a limit, paid every epoch
	ChargedUpTo { limit: FeeRate<Rate> },
}

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]

enum FeeEditor<AccountId> {
	Root,
	Account(AccountId),
}

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]

enum FeeBucket {
	/// Fees that are charged first, before any redemptions, investments,
	/// repayments or originations
	Top,
	// Future: AfterTranche(TrancheId)
}

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]

struct Fee<AccountId, Rate> {
	/// Account that the fees are sent to
	pub destination: AccountId,

	/// Account that can update this fee
	pub editor: FeeEditor<AccountId>,

	/// Amount of fees that can be charged
	pub amount: FeeAmount<Rate>,
}

// NOTE: Remark feature will be a separate feature in the future (post Pool Fees
// MVP). The following enum is necessary to deliver the MVP.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]

enum Remark<Hash, LoanId, Meta> {
	Loan { id: LoanId, meta: Meta },
	IpfsHash(Hash),
	Metadata(Meta),
}

type IpfsHash = [u8; 46];
