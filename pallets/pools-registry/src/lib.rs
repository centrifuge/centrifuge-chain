// Copyright 2022 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
#![cfg_attr(not(feature = "std"), no_std)]

use codec::HasCompact;
use frame_support::{pallet_prelude::*, scale_info::TypeInfo, BoundedVec};
use frame_system::pallet_prelude::*;
use polkadot_parachain::primitives::Id as ParachainId;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, BadOrigin, Zero},
    FixedPointOperand, WeakBoundedVec,
};
use xcm::{
    latest::MultiLocation,
    prelude::{GeneralKey, Parachain, X2},
    VersionedMultiLocation,
};

use orml_asset_registry::AssetMetadata;

use common_traits::Permissions;
use common_types::{CustomMetadata, Moment, PermissionScope, PoolRole, Role, XcmMetadata};
use sp_std::vec::Vec;

pub use pallet::*;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo)]
pub struct TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>
    where
        MaxTokenNameLength: Get<u32>,
        MaxTokenSymbolLength: Get<u32>,
{
    pub token_name: BoundedVec<u8, MaxTokenNameLength>,
    pub token_symbol: BoundedVec<u8, MaxTokenSymbolLength>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolMetadata<MetaSize>
    where
        MetaSize: Get<u32>,
{
    metadata: BoundedVec<u8, MetaSize>,
}

type PoolMetadataOf<T> = PoolMetadata<<T as Config>::MaxSizeMetadata>;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Balance: Member
        + Parameter
        + AtLeast32BitUnsigned
        + Default
        + Copy
        + MaxEncodedLen
        + FixedPointOperand
        + From<u64>
        + From<u128>
        + TypeInfo
        + TryInto<u64>;

        type PoolId: Member
        + Parameter
        + Default
        + Copy
        + HasCompact
        + MaxEncodedLen
        + core::fmt::Debug;

        type CurrencyId: Parameter + Copy;

        type Metadata: Eq
        + PartialEq
        + Member
        + Parameter
        + Default
        + Copy
        + HasCompact
        + MaxEncodedLen
        + core::fmt::Debug;

        type TrancheId: Member
        + Parameter
        + Default
        + Copy
        + MaxEncodedLen
        + TypeInfo
        + From<[u8; 16]>;

        /// Max size of Metadata
        #[pallet::constant]
        type MaxSizeMetadata: Get<u32> + Copy + Member + scale_info::TypeInfo;

        type Permission: Permissions<
            Self::AccountId,
            Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
            Role = Role<Self::TrancheId, Moment>,
            Error = DispatchError,
        >;

        /// Weight Information
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn get_pool_metadata)]
    pub(super) type PoolMetadata<T: Config> =
    StorageMap<_, Blake2_256, T::PoolId, PoolMetadataOf<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Pool metadata was set.
        MetadataSet {
            pool_id: T::PoolId,
            metadata: BoundedVec<u8, T::MaxSizeMetadata>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Invalid metadata passed
        BadMetadata,
        /// Pre-requirements for a TrancheUpdate are not met
        /// for example: Tranche changed but not its metadata or vice versa
        InvalidTrancheUpdate,
        /// No metada for the given currency found
        MetadataForCurrencyNotFound,
        /// No Metadata found for the given PoolId
        NoSuchPoolMetadata,
        /// The given tranche token name exceeds the length limit
        TrancheTokenNameTooLong,
        /// The given tranche symbol name exceeds the length limit
        TrancheSymbolNameTooLong,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Sets the IPFS hash for the pool metadata information.
        ///
        /// The caller must have the `PoolAdmin` role in order to
        /// invoke this extrinsic.
        #[pallet::weight(T::WeightInfo::set_metadata(metadata.len().try_into().unwrap_or(u32::MAX)))]
        pub fn set_metadata(
            origin: OriginFor<T>,
            pool_id: T::PoolId,
            metadata: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				BadOrigin,
			);

            let checked_metadata: BoundedVec<u8, T::MaxSizeMetadata> = metadata
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::BadMetadata)?;

            PoolMetadata::<T>::insert(
                pool_id,
                PoolMetadataOf::<T> {
                    metadata: checked_metadata,
                },
            );

            Ok(())
        }
    }
}

pub fn create_asset_metadata<Balance, CurrencyId>(
    decimals: u32,
    currency: CurrencyId,
    parachain_id: ParachainId,
    token_name: Vec<u8>,
    token_symbol: Vec<u8>,
) -> AssetMetadata<Balance, CustomMetadata>
    where
        Balance: Zero,
        CurrencyId: Encode,
        CustomMetadata: Parameter + Member + TypeInfo,
{
    let tranche_id = WeakBoundedVec::<u8, ConstU32<32>>::force_from(currency.encode(), None);

    AssetMetadata {
        decimals,
        name: token_name,
        symbol: token_symbol,
        existential_deposit: Zero::zero(),
        location: Some(VersionedMultiLocation::V1(MultiLocation {
            parents: 1,
            interior: X2(Parachain(parachain_id.into()), GeneralKey(tranche_id)),
        })),
        additional: CustomMetadata {
            mintable: false,
            permissioned: false,
            pool_currency: false,
            xcm: XcmMetadata {
                fee_per_second: None,
            },
        },
    }
}