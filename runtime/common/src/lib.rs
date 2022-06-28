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
pub use common_types::CurrencyId;

pub use apis::*;
pub use constants::*;
pub use impls::*;
pub use types::*;

pub mod apis;
mod fixed_point;
mod impls;

/// Common types for all runtimes
pub mod types {
	use codec::{CompactAs, Decode, Encode, MaxEncodedLen};
	use frame_support::traits::EnsureOneOf;
	use frame_system::EnsureRoot;
	use pallet_collective::EnsureProportionAtLeast;
	use scale_info::TypeInfo;
	#[cfg(feature = "std")]
	use serde::{Deserialize, Serialize};
	use sp_core::{H160, U256};
	use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, Verify};
	use sp_std::vec::Vec;

	// Ensure that origin is either Root or fallback to use EnsureOrigin `O`
	pub type EnsureRootOr<O> = EnsureOneOf<EnsureRoot<AccountId>, O>;

	/// The council
	pub type CouncilCollective = pallet_collective::Instance1;

	/// All council members must vote yes to create this origin.
	pub type AllOfCouncil = EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>;

	/// 1/2 of all council members must vote yes to create this origin.
	pub type HalfOfCouncil = EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>;

	/// 2/3 of all council members must vote yes to create this origin.
	pub type TwoThirdOfCouncil = EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>;

	/// 3/4 of all council members must vote yes to create this origin.
	pub type ThreeFourthOfCouncil = EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>;

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
	#[derive(Encode, Decode, Default, Copy, Clone, PartialEq, Eq, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	pub struct RegistryId(pub H160);

	// The id of an asset as it corresponds to the "token id" of a Centrifuge document.
	// A registry id is needed as well to uniquely identify an asset on-chain.
	#[derive(Encode, Decode, Default, Copy, Clone, PartialEq, Eq, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	pub struct TokenId(pub U256);

	/// A generic representation of a local address. A resource id points to this. It may be a
	/// registry id (20 bytes) or a fungible asset type (in the future). Constrained to 32 bytes just
	/// as an upper bound to store efficiently.
	#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Debug))]
	pub struct EthAddress(pub Bytes32);

	/// Rate with 27 precision fixed point decimal
	pub type Rate = crate::fixed_point::Rate;

	/// Amount with 18 precision fixed point decimal
	pub type Amount = crate::fixed_point::Amount;

	/// A representation of CollectionId for Uniques
	pub type CollectionId = u64;

	/// A representation of a tranche identifier
	pub type TrancheId = [u8; 16];

	/// A representation of a tranche weight, used to weight
	/// importance of a tranche
	#[derive(Encode, Decode, Copy, Debug, Default, Clone, PartialEq, Eq, TypeInfo, CompactAs)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub struct TrancheWeight(pub u128);

	/// A representation of ItemId for Uniques.
	#[derive(
		Encode,
		Decode,
		Default,
		Copy,
		Clone,
		PartialEq,
		Eq,
		CompactAs,
		Debug,
		MaxEncodedLen,
		TypeInfo,
	)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub struct ItemId(pub u128);

	/// A type describing our custom additional metadata stored in the OrmlAssetRegistry.
	#[derive(
		Clone,
		Copy,
		Default,
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
	pub struct CustomMetadata {
		/// XCM-related metadata.
		pub xcm: XcmMetadata,

		/// Whether an asset can be minted.
		/// When `true`, the right permissions will checked in the permissions
		/// pallet to authorize asset minting by an origin.
		pub mintable: bool,

		/// Whether an asset is _permissioned_, i.e., whether the asset can only
		/// be transferred from and to whitelisted accounts. When `true`, the
		/// right permissions will checked in the permissions pallet to authorize
		/// transfer between mutually allowed from and to accounts.
		pub permissioned: bool,

		/// Whether an asset can be used as a currency to fund Centrifuge Pools.
		pub pool_currency: bool,
	}

	#[derive(
		Clone,
		Copy,
		Default,
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
	pub struct XcmMetadata {
		/// The fee charged for every second that an XCM message takes to execute.
		/// When `None`, the `default_per_second` will be used instead.
		pub fee_per_second: Option<Balance>,
	}
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

	// The decimals for the tokens we handle natively in our runtimes.
	// Other tokens are registered in the orml_asset_registry and
	// their decimals can be found in their respective metadata.
	pub mod decimals {
		pub const NATIVE: u32 = 18;
		pub const AUSD: u32 = 12;
		pub const KSM: u32 = 12;
	}

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

/// Listing of parachains we integrate with.
/// For each parachain, we are interested in stating their parachain ID
/// as well as any of their token key ID that we possibly support in our
/// XCM configuration. These token keys are defined in said parachain
/// and must always match the value there defined, which is expected to
/// never change once defined since they help define the canonical id
/// of said tokens in the network, which is relevant for XCM transfers.
pub mod parachains {
	pub mod karura {
		pub const ID: u32 = 2000;
		pub const KUSD_KEY: &[u8] = &[0, 129];
	}

	pub mod altair {
		pub const ID: u32 = 2088;
		pub const AIR_KEY: &[u8] = &[0, 1];
	}
}

pub mod xcm_fees {
	use frame_support::weights::constants::{ExtrinsicBaseWeight, WEIGHT_PER_SECOND};

	use super::{decimals, Balance};

	// The fee cost per second for transferring the native token in cents.
	pub fn native_per_second() -> Balance {
		default_per_second(decimals::NATIVE)
	}

	pub fn ksm_per_second() -> Balance {
		default_per_second(decimals::KSM) / 50
	}

	pub fn default_per_second(decimals: u32) -> Balance {
		let base_weight = Balance::from(ExtrinsicBaseWeight::get());
		let default_per_second = (WEIGHT_PER_SECOND as u128) / base_weight;
		default_per_second * base_fee(decimals)
	}

	fn base_fee(decimals: u32) -> Balance {
		dollar(decimals)
			// cents
			.saturating_div(100)
			// a tenth of a cent
			.saturating_div(10)
	}

	pub fn dollar(decimals: u32) -> Balance {
		10u128.saturating_pow(decimals.into())
	}
}
