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

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]
// Allow things like `1 * CFG`
#![allow(clippy::identity_op)]

mod impls;

pub use constants::*;
pub use types::*;

/// Common types for all runtimes
pub mod types {
	use codec::{CompactAs, Decode, Encode, MaxEncodedLen};
	use frame_support::traits::EitherOfDiverse;
	use frame_system::EnsureRoot;
	use pallet_collective::EnsureProportionAtLeast;
	use scale_info::TypeInfo;
	#[cfg(feature = "std")]
	use serde::{Deserialize, Serialize};
	use sp_core::{H160, U256};
	use sp_runtime::{
		traits::{self, BlakeTwo256, IdentifyAccount, Verify},
		OpaqueExtrinsic,
	};
	use sp_std::vec::Vec;

	/// PoolId type we use.
	pub type PoolId = u64;

	/// OrderId type we use to identify order per epoch.
	pub type OrderId = u64;

	/// EpochId type we use to identify epochs in our revolving pools
	pub type PoolEpochId = u32;

	// Ensure that origin is either Root or fallback to use EnsureOrigin `O`
	pub type EnsureRootOr<O> = EitherOfDiverse<EnsureRoot<AccountId>, O>;

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
	pub type Hash = <BlakeTwo256 as traits::Hash>::Output;

	/// The hashing algorithm used by the chain
	///
	/// NOTE: Must never change
	pub type Hashing = BlakeTwo256;

	/// A generic block for the node to use, as we can not commit to
	/// a specific Extrinsic format at this point. Runtimes will ensure
	/// Extrinsic are correctly decoded.
	pub type Block = sp_runtime::generic::Block<Header, OpaqueExtrinsic>;

	/// Block header type as expected by this runtime.
	pub type Header = sp_runtime::generic::Header<BlockNumber, Hashing>;

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

	/// A representation of a loan identifier
	pub type LoanId = u64;
}

/// Common constants for all runtimes
pub mod constants {
	use cumulus_primitives_core::relay_chain::v2::MAX_POV_SIZE;
	use frame_support::weights::{constants::WEIGHT_PER_SECOND, Weight};
	use sp_runtime::Perbill;

	use super::types::{Balance, BlockNumber};

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
	pub const MAXIMUM_BLOCK_WEIGHT: Weight = WEIGHT_PER_SECOND
		.saturating_div(2)
		.set_proof_size(MAX_POV_SIZE as u64);

	pub const MICRO_CFG: Balance = 1_000_000_000_000; // 10−6 	0.000001
	pub const MILLI_CFG: Balance = 1_000 * MICRO_CFG; // 10−3 	0.001
	pub const CENTI_CFG: Balance = 10 * MILLI_CFG; // 10−2 	0.01
	pub const CFG: Balance = 100 * CENTI_CFG;

	// The decimals for the tokens we handle natively in our runtimes.
	// Other tokens are registered in the orml_asset_registry and
	// their decimals can be found in their respective metadata.
	pub mod currency_decimals {
		pub const NATIVE: u32 = 18;
		pub const AUSD: u32 = 12;
		pub const KSM: u32 = 12;
	}

	/// Minimum vesting amount, in CFG/AIR
	pub const MIN_VESTING: Balance = 10;

	/// Value for a not specified fee key.
	pub const DEFAULT_FEE_VALUE: Balance = 1 * CFG;

	/// % of fee addressed to the Treasury. The reminder % will be for the block author.
	pub const TREASURY_FEE_RATIO: Perbill = Perbill::from_percent(80);

	/// The max length allowed for a tranche token name
	pub const MAX_TOKEN_NAME_LENGTH_BYTES: u32 = 128;

	/// The max length allowed for a tranche token symbol
	pub const MAX_TOKEN_SYMBOL_LENGTH_BYTES: u32 = 32;

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
	pub mod kusama {
		pub mod karura {
			pub const ID: u32 = 2000;
			pub const AUSD_KEY: &[u8] = &[0, 129];
		}

		pub mod altair {
			pub const ID: u32 = 2088;
			pub const AIR_KEY: &[u8] = &[0, 1];
		}
	}

	pub mod polkadot {
		pub mod acala {
			pub const ID: u32 = 2000;
			pub const AUSD_KEY: &[u8] = &[0, 1];
		}

		pub mod centrifuge {
			pub const ID: u32 = 2031;
			pub const CFG_KEY: &[u8] = &[0, 1];
		}
	}

	pub mod rococo {
		pub mod rocksmine {
			pub const ID: u32 = 1000;
			pub mod usdt {
				pub const PALLET_INSTANCE: u8 = 50;
				pub const GENERAL_INDEX: u128 = 1984;
			}
		}

		pub mod acala {
			pub const ID: u32 = 2000;
			pub const AUSD_KEY: &[u8] = &[0, 129];
		}
	}
}
