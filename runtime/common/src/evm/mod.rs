// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::AccountId;
use codec::{Decode, Encode};
use pallet_ethereum::{Transaction, TransactionAction};
use pallet_evm::AddressMapping;
use sp_core::H160;
use sp_runtime::{traits::AccountIdConversion, Permill};

pub mod precompile;

#[derive(Encode, Decode, Default)]
struct Account(H160);

impl sp_runtime::TypeId for Account {
	const TYPE_ID: [u8; 4] = *b"ETH\0";
}

pub struct ExpandedAddressMapping;

// Ethereum chain interactions are done with a 20-byte account ID. But
// Substrate uses a 32-byte account ID. This implementation stretches
// a 20-byte account into a 32-byte account by adding a tag and a few
// zero bytes.
impl AddressMapping<AccountId> for ExpandedAddressMapping {
	fn into_account_id(address: H160) -> AccountId {
		Account(address).into_account_truncating()
	}
}

pub struct BaseFeeThreshold;

// Set our ideal block fullness to 50%. Anything between 50%-100% will cause the
// gas fee to increase. Anything from 0%-50% will cause the gas fee to decrease.
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
	fn lower() -> Permill {
		Permill::zero()
	}

	fn ideal() -> Permill {
		Permill::from_parts(500_000)
	}

	fn upper() -> Permill {
		Permill::from_parts(1_000_000)
	}
}

pub trait GetTransactionAction {
	fn action(&self) -> TransactionAction;
}

impl GetTransactionAction for Transaction {
	fn action(&self) -> TransactionAction {
		match self {
			Transaction::Legacy(transaction) => transaction.action,
			Transaction::EIP2930(transaction) => transaction.action,
			Transaction::EIP1559(transaction) => transaction.action,
		}
	}
}
