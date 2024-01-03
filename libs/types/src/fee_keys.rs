// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

use frame_support::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::cmp::PartialEq;

/// Different fees keys available.
/// Each variant represents a balance previously determined and configured.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum FeeKey {
	/// Key to identify the balance reserved for the author.
	/// See more at `pallet-anchors`
	AnchorsCommit,

	/// Key to identify the balance reserved for the deposit.
	/// See more at `pallet-anchors`
	AnchorsPreCommit,

	/// Key to identify the balance reserved for burning.
	/// See more at `pallet-bridge`
	BridgeNativeTransfer,

	/// Key to identify the balance reserved for burning.
	/// See more at `pallet-nft`
	NftProofValidation,

	/// Key to identify the balance reserved for Allowances.
	/// See more at `pallet-transfer-allowlist`
	AllowanceCreation,

	/// Key to identify the balance reserved for Orderbook Orders
	/// See more at `pallet-order-book`
	OrderBookOrderCreation,
}

/// Only needed for initializing the runtime benchmark with some value.
#[cfg(feature = "runtime-benchmarks")]
impl Default for FeeKey {
	fn default() -> Self {
		FeeKey::AnchorsCommit
	}
}

pub type Fee = cfg_traits::fees::Fee<cfg_primitives::Balance, FeeKey>;
