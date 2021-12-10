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

///! The xcm-chain-adapter Pallet.
///! This pallet provides means of sending XCMs to another `MultiLocation` and allows
///! for other pallets to listen in on receiving responses.
#![cfg_attr(not(feature = "std"), no_std)]

pub mod traits;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod mock;
mod benchmarking;
pub mod weights;

pub use pallet::*;
pub use weights::*;

use crate::traits::XcmSink;
use cumulus_primitives_core::{XcmpMessageHandler, DmpMessageHandler, XcmpMessageSource};
use xcm::v1::SendXcm;
use xcm::v1::ExecuteXcm;
use xcm::opaque::VersionedXcm;
use xcm::VersionedMultiLocation;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use crate::traits::{XcmResponseHandler};
    use cumulus_primitives_core::UpwardMessageSender;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The XCM format that this adapter uses for incoming XCMs.
        ///
        /// As XCM (in the future) will be generic over Call, this will be
        /// `type XcmIn: TryFrom<VersionedXcm<Self::Call>>` at this point.
        type XcmIn: TryFrom<VersionedXcm>;

        /// The UMP sink
        type UmpSink: UpwardMessageSender;

        /// XCMP queue. The queue that implements the actual XCMP standard. I.e. the low-level
        /// queue that we stuff in our XCMs so that they are XCMP compatible.
        type Xcmp: XcmpMessageSource + SendXcm;

        type Executor: ExecuteXcm<()>;

        /// The handlers that will be called, when a response for an Xcm has arrived
        type XcmResponseHandler: XcmResponseHandler;

        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Type representing the weight of this pallet
        type WeightInfo: WeightInfo;
    }

    // The genesis config type.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub initial_state: Vec<T::ValidatorId>,
    }

    // The default value for the genesis config type.
    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                initial_state: vec![],
            }
        }
    }

    // The build of genesis for the pallet.
    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.initial_state
                .iter()
                .for_each(|id| <Allowlist<T>>::insert(id, ()));
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
    }

    #[pallet::error]
    pub enum Error<T> {
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(<T as pallet::Config>::WeightInfo::add())]
        pub fn add(origin: OriginFor<T>, collator_id: T::ValidatorId) -> DispatchResult {
        }
    }
}


impl<T: Config> Pallet<T> {

}

impl<T: Config> OnValidationData for Pallet<T> {

}

impl<T: Config> XcmpSink for Pallet<T> {
    type Xcm = VersionedXcm;
    type Receiver = VersionedMultilocation;

    fn send(msg: Self::Xcm, recv: Self::Receiver) {
        todo!("Determine the right version and use appropriate send channel");
    }
}

impl<T: Config> XcmpMessageHandler for Pallet<T> {

}

impl<T: Config> DmpMessageHandler for Pallet<T> {

}

impl<T: Config> XcmpMessageSource for Pallet<T> {

}

impl<T: Config> ExecuteXcm for Pallet<T> {

}

impl<T: Config> SendXcm for Pallet<T> {

}

impl<T: Config> VersionWrapper for Pallet<T> {

}