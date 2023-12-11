// Copyright 2023 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_types::pools::FeeBucket;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::TypeInfo;

use crate::{Config, PoolFeeOf};

/// Represents a fee which will be disbursed during epoch execution.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub struct DisbursingFee<AccountId, Balance, FeeId> {
	pub amount: Balance,
	pub destination: AccountId,
	pub fee_id: FeeId,
}

/// Represents pool changes which might require to complete further guarding
/// checks.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
#[scale_info(skip_type_params(T))]
pub enum Change<T: Config> {
	AppendFee(FeeBucket, PoolFeeOf<T>),
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
