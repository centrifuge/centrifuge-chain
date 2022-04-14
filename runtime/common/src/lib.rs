// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! # Common types and primitives used for Centrifuge chain runtime.

#![cfg_attr(not(feature = "std"), no_std)]

pub use apis::*;
pub use constants::*;
pub use impls::*;
pub use types::*;

mod fixed_point;
mod impls;

pub use common_types::CurrencyId;

pub mod apis {
	use node_primitives::{BlockNumber, Hash};
	use pallet_anchors::AnchorData;
	use sp_api::decl_runtime_apis;

	decl_runtime_apis! {
		/// The API to query anchoring info.
		pub trait AnchorApi {
			fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>>;
		}
	}
}

/// Common types for all runtimes
pub mod types {
	use frame_support::traits::EnsureOneOf;
	use frame_system::EnsureRoot;
	use scale_info::TypeInfo;
	#[cfg(feature = "std")]
	use serde::{Deserialize, Serialize};
	use sp_core::{H160, U256};
	use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, Verify};
	use sp_std::vec::Vec;

	pub type EnsureRootOr<O> = EnsureOneOf<EnsureRoot<AccountId>, O>;

	/// An index to a block.
	pub type BlockNumber = u32;

	/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
	pub type Signature = sp_runtime::MultiSignature;

	/// Some way of identifying an account on the chain. We intentionally make it equivalent
	/// to the public key of our transaction signing scheme.
	pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

	/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
	/// never know...
	pub type AccountIndex = u32;

	/// The address format for describing accounts.
	pub type Address = sp_runtime::MultiAddress<AccountId, ()>;

	/// Balance of an account.
	pub type Balance = u128;

	/// IBalance is the signed version of the Balance for orml tokens
	pub type IBalance = i128;

	/// Index of a transaction in the chain.
	pub type Index = u32;

	/// A hash of some data used by the chain.
	pub type Hash = sp_core::H256;

	/// Block header type as expected by this runtime.
	pub type Header = sp_runtime::generic::Header<BlockNumber, BlakeTwo256>;

	/// Aura consensus authority.
	pub type AuraId = sp_consensus_aura::sr25519::AuthorityId;

	/// Moment type
	pub type Moment = u64;

	// A vector of bytes, conveniently named like it is in Solidity.
	pub type Bytes = Vec<u8>;

	// A 32 bytes fixed-size array.
	pub type Bytes32 = FixedArray<u8, 32>;

	// Fixed-size array of given typed elements.
	pub type FixedArray<T, const S: usize> = [T; S];

	// A cryptographic salt to be combined with a value before hashing.
	pub type Salt = FixedArray<u8, 32>;

	/// A representation of registryID.
	#[derive(codec::Encode, codec::Decode, Default, Copy, Clone, PartialEq, Eq, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	pub struct RegistryId(pub H160);

	// The id of an asset as it corresponds to the "token id" of a Centrifuge document.
	// A registry id is needed as well to uniquely identify an asset on-chain.
	#[derive(codec::Encode, codec::Decode, Default, Copy, Clone, PartialEq, Eq, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	pub struct TokenId(pub U256);

	/// A generic representation of a local address. A resource id points to this. It may be a
	/// registry id (20 bytes) or a fungible asset type (in the future). Constrained to 32 bytes just
	/// as an upper bound to store efficiently.
	#[derive(codec::Encode, codec::Decode, Default, Clone, PartialEq, Eq, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Debug))]
	pub struct EthAddress(pub Bytes32);

	/// Rate with 27 precision fixed point decimal
	pub type Rate = crate::fixed_point::Rate;

	/// Amount with 18 precision fixed point decimal
	pub type Amount = crate::fixed_point::Amount;

	/// A representation of ClassId for Uniques
	pub type ClassId = u64;

	/// A representation of a tranche identifier
	pub type TrancheId = [u8; 16];

	/// A representation of a tranche weight, used to weight
	/// importance of a tranche
	#[derive(
		codec::Encode,
		codec::Decode,
		Copy,
		Debug,
		Default,
		Clone,
		PartialEq,
		Eq,
		TypeInfo,
		codec::CompactAs,
	)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub struct TrancheWeight(pub u128);

	/// A representation of InstanceId for Uniques.
	#[derive(
		codec::Encode,
		codec::Decode,
		Default,
		Copy,
		Clone,
		PartialEq,
		Eq,
		codec::CompactAs,
		Debug,
		codec::MaxEncodedLen,
		TypeInfo,
	)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub struct InstanceId(pub u128);
}

/// Common constants for all runtimes
pub mod constants {
	use super::types::BlockNumber;
	use frame_support::weights::{constants::WEIGHT_PER_SECOND, Weight};
	use node_primitives::Balance;
	use sp_runtime::Perbill;

	/// This determines the average expected block time that we are targeting. Blocks will be
	/// produced at a minimum duration defined by `SLOT_DURATION`. `SLOT_DURATION` is picked up by
	/// `pallet_timestamp` which is in turn picked up by `pallet_aura` to implement `fn
	/// slot_duration()`.
	///
	/// Change this to adjust the block time.
	pub const MILLISECS_PER_BLOCK: u64 = 12000;
	pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

	// Time is measured by number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;

	/// Milliseconds per day
	pub const MILLISECS_PER_DAY: u64 = SECONDS_PER_DAY * 1000;

	// Seconds units
	pub const SECONDS_PER_MINUTE: u64 = 60;
	pub const SECONDS_PER_HOUR: u64 = SECONDS_PER_MINUTE * 60;
	pub const SECONDS_PER_DAY: u64 = SECONDS_PER_HOUR * 24;
	pub const SECONDS_PER_YEAR: u64 = SECONDS_PER_DAY * 365;

	/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
	/// used to limit the maximal weight of a single extrinsic.
	pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);
	/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
	/// Operational  extrinsics.
	pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

	/// We allow for 0.5 seconds of compute with a 6 second average block time.
	pub const MAXIMUM_BLOCK_WEIGHT: Weight = WEIGHT_PER_SECOND / 2;

	pub const MICRO_CFG: Balance = 1_000_000_000_000; // 10−6 	0.000001
	pub const MILLI_CFG: Balance = 1_000 * MICRO_CFG; // 10−3 	0.001
	pub const CENTI_CFG: Balance = 10 * MILLI_CFG; // 10−2 	0.01
	pub const CFG: Balance = 100 * CENTI_CFG;

	/// Minimum vesting amount, in CFG/AIR
	pub const MIN_VESTING: Balance = 10;

	/// Additional fee charged when moving native tokens to target chains (in CFGs).
	pub const NATIVE_TOKEN_TRANSFER_FEE: Balance = 2000 * CFG;

	/// Additional fee charged when moving NFTs to target chains (in CFGs).
	pub const NFT_TOKEN_TRANSFER_FEE: Balance = 20 * CFG;

	/// Additional fee charged when validating NFT proofs
	pub const NFT_PROOF_VALIDATION_FEE: Balance = 10 * CFG;

	// Represents the protobuf encoding - "NFTS". All Centrifuge documents are formatted in this way.
	/// These are pre/appended to the registry id before being set as a [RegistryInfo] field in [create_registry].
	pub const NFTS_PREFIX: &'static [u8] = &[1, 0, 0, 0, 0, 0, 0, 20];

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 15 * CENTI_CFG + (bytes as Balance) * 6 * CENTI_CFG
	}
}

pub mod parachains {
	pub mod karura {
		pub const ID: u32 = 2000;
		pub const KUSD_KEY: &[u8] = &[0, 129];
	}
}

pub mod xcm_fees {
	use common_traits::TokenMetadata;
	use common_types::CurrencyId;
	use frame_support::weights::constants::{ExtrinsicBaseWeight, WEIGHT_PER_SECOND};

	use super::types::Balance;
	use super::CENTI_CFG as CENTI_CURRENCY;

	pub fn base_tx_in_air() -> Balance {
		CENTI_CURRENCY / 10
	}

	// The fee cost per second for transferring the native token in cents.
	pub fn native_per_second() -> Balance {
		base_tx_per_second(CurrencyId::Native)
	}

	pub fn ksm_per_second() -> Balance {
		base_tx_per_second(CurrencyId::KSM) / 50
	}

	fn base_tx_per_second(currency: CurrencyId) -> Balance {
		let base_weight = Balance::from(ExtrinsicBaseWeight::get());
		let base_tx_per_second = (WEIGHT_PER_SECOND as u128) / base_weight;
		base_tx_per_second * base_tx(currency)
	}

	fn base_tx(currency: CurrencyId) -> Balance {
		cent(currency) / 10
	}

	pub fn dollar(currency_id: common_types::CurrencyId) -> Balance {
		10u128.saturating_pow(currency_id.decimals().into())
	}

	pub fn cent(currency_id: CurrencyId) -> Balance {
		dollar(currency_id) / 100
	}
}
