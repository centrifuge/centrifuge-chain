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

pub mod conversion;
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

	/// Alias to 512-bit hash when used in the context of a transaction
	/// signature on the chain.
	pub type Signature = sp_runtime::MultiSignature;

	/// Some way of identifying an account on the chain. We intentionally make
	/// it equivalent to the public key of our transaction signing scheme.
	pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

	/// The type for looking up accounts. We don't expect more than 4 billion of
	/// them, but you never know...
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

	// The id of an asset as it corresponds to the "token id" of a Centrifuge
	// document. A registry id is needed as well to uniquely identify an asset
	// on-chain.
	#[derive(Encode, Decode, Default, Copy, Clone, PartialEq, Eq, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
	pub struct TokenId(pub U256);

	/// A generic representation of a local address. A resource id points to
	/// this. It may be a registry id (20 bytes) or a fungible asset type (in
	/// the future). Constrained to 32 bytes just as an upper bound to store
	/// efficiently.
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

	/// The type for indexing pallets on a Substrate runtime
	pub type PalletIndex = u8;
}

/// Common constants for all runtimes
pub mod constants {
	use cumulus_primitives_core::relay_chain::MAX_POV_SIZE;
	use frame_support::weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight};
	use sp_runtime::Perbill;

	use super::types::{Balance, BlockNumber};

	/// This determines the average expected block time that we are targeting.
	/// Blocks will be produced at a minimum duration defined by
	/// `SLOT_DURATION`. `SLOT_DURATION` is picked up by `pallet_timestamp`
	/// which is in turn picked up by `pallet_aura` to implement `fn
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
	pub const SECONDS_PER_WEEK: u64 = SECONDS_PER_DAY * 7;
	pub const SECONDS_PER_MONTH: u64 = SECONDS_PER_DAY * 30;
	pub const SECONDS_PER_YEAR: u64 = SECONDS_PER_DAY * 365;

	/// We assume that ~5% of the block weight is consumed by `on_initialize`
	/// handlers. This is used to limit the maximal weight of a single
	/// extrinsic.
	pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);
	/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest
	/// can be used by Operational  extrinsics.
	pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

	/// We allow for 0.5 seconds of compute with a 6 second average block time.
	pub const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_ref_time(WEIGHT_REF_TIME_PER_SECOND)
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

	/// % of fee addressed to the Treasury. The reminder % will be for the block
	/// author.
	pub const TREASURY_FEE_RATIO: Perbill = Perbill::from_percent(80);

	/// The max length allowed for a tranche token name
	pub const MAX_TOKEN_NAME_LENGTH_BYTES: u32 = 128;

	/// The max length allowed for a tranche token symbol
	pub const MAX_TOKEN_SYMBOL_LENGTH_BYTES: u32 = 32;

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 15 * CENTI_CFG + (bytes as Balance) * 6 * CENTI_CFG
	}

	/// Unhashed 36-bytes prefix for currencies managed by LiquidityPools.
	pub const GENERAL_CURRENCY_INDEX_PREFIX: [u8; 36] = *b"CentrifugeGeneralCurrencyIndexPrefix";

	/// Transaction recovery ID used for generating a signature in the Ethereum
	/// Transaction pallet. As per:
	/// <https://github.com/PureStake/moonbeam/blob/fb63014a5e487f17e31283776e4f6b0befd009a2/primitives/xcm/src/ethereum_xcm.rs#L167>
	pub const TRANSACTION_RECOVERY_ID: u64 = 42;
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

pub mod liquidity_pools {
	/// The hashed prefix for currencies managed by LiquidityPools.
	pub struct GeneralCurrencyPrefix;

	impl sp_core::Get<[u8; 12]> for GeneralCurrencyPrefix {
		fn get() -> [u8; 12] {
			let hash: [u8; 16] = frame_support::sp_io::hashing::blake2_128(
				&crate::constants::GENERAL_CURRENCY_INDEX_PREFIX,
			);
			let (trimmed, _) = hash.split_at(12);

			trimmed
				.try_into()
				.expect("Should not fail to trim 16-length byte array to length 12")
		}
	}
}

pub mod xcm {
	use codec::{Compact, Encode};
	use sp_core::blake2_256;
	use sp_std::{borrow::Borrow, marker::PhantomData, vec::Vec};
	use xcm::prelude::{
		AccountId32, AccountKey20, Here, MultiLocation, PalletInstance, Parachain, X1,
	};
	use xcm_executor::traits::Convert;

	/// NOTE: Copied from https://github.com/moonbeam-foundation/polkadot/blob/d83bb6cc7d7c93ead2fd3cafce0e268fd3f6b9bc/xcm/xcm-builder/src/location_conversion.rs#L25C1-L68C2
	///
	/// temporary struct that mimics the behavior of the upstream type that we
	/// will move to once we update this repository to Polkadot 0.9.43+:
	/// HashedDescription<AccountId, DescribeFamily<DescribeAllTerminal>>
	pub struct HashedDescriptionDescribeFamilyAllTerminal<AccountId>(PhantomData<AccountId>);
	impl<AccountId: From<[u8; 32]> + Clone> HashedDescriptionDescribeFamilyAllTerminal<AccountId> {
		fn describe_location_suffix(l: &MultiLocation) -> Result<Vec<u8>, ()> {
			match (l.parents, &l.interior) {
				(0, Here) => Ok(Vec::new()),
				(0, X1(PalletInstance(i))) => {
					Ok((b"Pallet", Compact::<u32>::from(*i as u32)).encode())
				}
				(0, X1(AccountId32 { id, .. })) => Ok((b"AccountId32", id).encode()),
				(0, X1(AccountKey20 { key, .. })) => Ok((b"AccountKey20", key).encode()),
				_ => return Err(()),
			}
		}
	}

	impl<AccountId: From<[u8; 32]> + Clone> Convert<MultiLocation, AccountId>
		for HashedDescriptionDescribeFamilyAllTerminal<AccountId>
	{
		fn convert_ref(location: impl Borrow<MultiLocation>) -> Result<AccountId, ()> {
			let l = location.borrow();
			let to_hash = match (l.parents, l.interior.first()) {
				(0, Some(Parachain(index))) => {
					let tail = l.interior.split_first().0;
					let interior = Self::describe_location_suffix(&tail.into())?;
					(b"ChildChain", Compact::<u32>::from(*index), interior).encode()
				}
				(1, Some(Parachain(index))) => {
					let tail = l.interior.split_first().0;
					let interior = Self::describe_location_suffix(&tail.into())?;
					(b"SiblingChain", Compact::<u32>::from(*index), interior).encode()
				}
				(1, _) => {
					let tail = l.interior.into();
					let interior = Self::describe_location_suffix(&tail)?;
					(b"ParentChain", interior).encode()
				}
				_ => return Err(()),
			};
			Ok(blake2_256(&to_hash).into())
		}

		fn reverse_ref(_: impl Borrow<AccountId>) -> Result<MultiLocation, ()> {
			Err(())
		}
	}


	#[test]
	fn test_hashed_family_all_terminal_converter() {
		use xcm::prelude::X2;

		type Converter<AccountId> = HashedDescriptionDescribeFamilyAllTerminal<AccountId>;

		assert_eq!(
			[
				129, 211, 14, 6, 146, 54, 225, 200, 135, 103, 248, 244, 125, 112, 53, 133,
				91, 42, 215, 236, 154, 199, 191, 208, 110, 148, 223, 55, 92, 216, 250, 34
			],
			Converter::<[u8; 32]>::convert(MultiLocation {
				parents: 0,
				interior: X2(Parachain(1), AccountId32 { network: None, id: [0u8; 32] }),
			}).unwrap()
		);
		assert_eq!(
			[
				17, 142, 105, 253, 199, 34, 43, 136, 155, 48, 12, 137, 155, 219, 155, 110,
				93, 181, 93, 252, 124, 60, 250, 195, 229, 86, 31, 220, 121, 111, 254, 252
			],
			Converter::<[u8; 32]>::convert(MultiLocation {
				parents: 1,
				interior: X2(Parachain(1), AccountId32 { network: None, id: [0u8; 32] }),
			}).unwrap()
		);
		assert_eq!(
			[
				237, 65, 190, 49, 53, 182, 196, 183, 151, 24, 214, 23, 72, 244, 235, 87,
				187, 67, 52, 122, 195, 192, 10, 58, 253, 49, 0, 112, 175, 224, 125, 66
			],
			Converter::<[u8; 32]>::convert(MultiLocation {
				parents: 0,
				interior: X2(Parachain(1), AccountKey20 { network: None, key: [0u8; 20] }),
			}).unwrap()
		);
		assert_eq!(
			[
				226, 225, 225, 162, 254, 156, 113, 95, 68, 155, 160, 118, 126, 18, 166, 132,
				144, 19, 8, 204, 228, 112, 164, 189, 179, 124, 249, 1, 168, 110, 151, 50
			],
			Converter::<[u8; 32]>::convert(MultiLocation {
				parents: 1,
				interior: X2(Parachain(1), AccountKey20 { network: None, key: [0u8; 20] }),
			}).unwrap()
		);
		assert_eq!(
			[
				254, 186, 179, 229, 13, 24, 84, 36, 84, 35, 64, 95, 114, 136, 62, 69, 247,
				74, 215, 104, 121, 114, 53, 6, 124, 46, 42, 245, 121, 197, 12, 208
			],
			Converter::<[u8; 32]>::convert(MultiLocation {
				parents: 1,
				interior: X2(Parachain(2), PalletInstance(3)),
			}).unwrap()
		);
		assert_eq!(
			[
				217, 56, 0, 36, 228, 154, 250, 26, 200, 156, 1, 39, 254, 162, 16, 187, 107,
				67, 27, 16, 218, 254, 250, 184, 6, 27, 216, 138, 194, 93, 23, 165
			],
			Converter::<[u8; 32]>::convert(MultiLocation {
				parents: 1,
				interior: Here,
			}).unwrap()
		);
	}
}
