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

pub type UpgradeCentrifuge1020 = ();

#[cfg(test)]
mod tests {
	use cfg_primitives::TrancheId;
	use cfg_types::{tokens as before, tokens::StakingCurrency};
	use codec::Encode;
	use hex::FromHex;

	mod after {
		use cfg_primitives::{PoolId, TrancheId};
		use cfg_types::tokens::{ForeignAssetId, StakingCurrency};
		use codec::{Decode, Encode, MaxEncodedLen};
		use scale_info::TypeInfo;

		#[derive(
			Clone,
			Copy,
			PartialOrd,
			Ord,
			PartialEq,
			Eq,
			Debug,
			Encode,
			Decode,
			TypeInfo,
			MaxEncodedLen,
		)]
		pub enum CurrencyId {
			/// The Native token, representing AIR in Altair and CFG in
			/// Centrifuge.
			#[codec(index = 0)]
			Native,

			/// A Tranche token
			#[codec(index = 1)]
			Tranche(PoolId, TrancheId),

			/// A foreign asset
			#[codec(index = 4)]
			ForeignAsset(ForeignAssetId),

			/// A staking token
			#[codec(index = 5)]
			Staking(StakingCurrency),
		}
	}

	#[test]
	fn encode_equality() {
		assert_eq!(
			before::CurrencyId::Native.encode(),
			after::CurrencyId::Native.encode()
		);
		assert_eq!(after::CurrencyId::Native.encode(), vec![0]);

		assert_eq!(
			before::CurrencyId::Tranche(33, default_tranche_id()).encode(),
			after::CurrencyId::Tranche(33, default_tranche_id()).encode()
		);
		assert_eq!(
			after::CurrencyId::Tranche(33, default_tranche_id()).encode(),
			vec![
				1, 33, 0, 0, 0, 0, 0, 0, 0, 129, 26, 205, 91, 63, 23, 192, 104, 65, 199, 228, 30,
				158, 4, 203, 27
			]
		);

		assert_eq!(
			before::CurrencyId::ForeignAsset(91).encode(),
			after::CurrencyId::ForeignAsset(91).encode()
		);
		assert_eq!(
			after::CurrencyId::ForeignAsset(91).encode(),
			vec![4, 91, 0, 0, 0]
		);

		assert_eq!(
			before::CurrencyId::Staking(StakingCurrency::BlockRewards).encode(),
			after::CurrencyId::Staking(StakingCurrency::BlockRewards).encode()
		);
		assert_eq!(
			after::CurrencyId::Staking(StakingCurrency::BlockRewards).encode(),
			vec![5, 0]
		);
	}

	fn default_tranche_id() -> TrancheId {
		<[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b")
			.expect("Should be valid tranche id")
	}
}