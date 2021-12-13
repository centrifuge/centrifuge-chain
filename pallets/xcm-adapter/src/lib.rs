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
pub mod queues;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod mock;
mod benchmarking;
pub mod weights;

pub use pallet::*;
pub use weights::*;

use crate::traits::XcmSink;
use cumulus_primitives_core::{XcmpMessageHandler, DmpMessageHandler, XcmpMessageSource, OnValidationData, PersistedValidationData};
use xcm::v2::SendXcm;
use xcm::v2::ExecuteXcm;
use xcm::opaque::VersionedXcm;
use xcm::{VersionedMultiLocation, WrapVersion};
use cumulus_primitives_core::relay_chain::v1::Id;
use xcm::latest::{Xcm, Outcome, MultiLocation, SendResult};


#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use crate::traits::{XcmResponseHandler, XcmRouter};
    use cumulus_primitives_core::UpwardMessageSender;
    use frame_support::sp_runtime::sp_std::convert::TryFrom;

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

        /// The used Multilocation type for incoming XCMs
        type Sender: TryFrom<VersionedMultilocation>;

        /// The XCM format that this adapter uses for incoming XCMs.
        ///
        /// As XCM (in the future) will be generic over Call, this will be
        /// `type XcmIn: TryFrom<VersionedXcm<Call>>` at this point. Where
        /// Call will be some enum that belongs to one or multiple other chains.
        type XcmOut: TryFrom<VersionedXcm>;

        /// The used Receiving Mulltilocation type
        type Receiver: TryFrom<VersionedMultiLocation>;

        /// The UMP sink. Outgoing messages will be processed here.
        type UmpSink: UpwardMessageSender;

        /// The DMP source. Incoming downward messages will be processed here
        type DmpSource: DmpMessageHandler;

        /// XCMP queue for incoming xcmps. The queue that implements the actual XCMP standard. I.e. the low-level
        /// queue that we stuff in our XCMs so that they are XCMP compatible.
        type XcmpSource: XcmpMessageHandler;

        /// XCMP queue for outgoing xcmps. The queue that implements the actual XCMP standard. I.e. the low-level
        /// queue that we stuff in our XCMs so that they are XCMP compatible.
        type XcmpSink: XcmpMessageSource;


        /// The router. This object takes care of actually routing to the right destinations.
        /// I.e. this is deciding wether to put something in an UMP-queue or an XCMP-queue.
        type Router: XcmRouter<Xcm = Self::XcmOut, Receiver = Self::Receiver>;

        /// The actial executor. We proxy the executor to provide information about
        /// incoming xcms to the xcm response handlers
        type ExecutorIn: ExecuteXcm<()>;

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
    fn on_validation_data(data: &PersistedValidationData) {
        todo!()
    }
}

impl<T: Config> XcmpSink for Pallet<T> {
    type Xcm = Self::XcmOut;
    type Receiver = <Self as Config>::Receiver;

    fn send(msg: Self::Xcm, recv: Self::Receiver) {
        todo!("Determine the right version and use appropriate send channel");
    }
}

impl<T: Config> XcmpMessageHandler for Pallet<T> {
    fn handle_xcmp_messages<'a, I: Iterator<Item=(Id, RelayChainBlockNumber, &'a [u8])>>(iter: I, max_weight: u64) -> u64 {
        todo!()
        // Implement a proxy for the actual Self::XcmpSource
        // This is usefull for:
        // * filtering out if response for the incoming xcm is wanted
    }
}

impl<T: Config> DmpMessageHandler for Pallet<T> {
    fn handle_dmp_messages(iter: impl Iterator<Item=(u32, Vec<u8>)>, max_weight: u64) -> u64 {
        todo!()
        // Implement a proxy for the actual Self::DmpSource
        // This is usefull for:
        // * filtering out if response for the incoming xcm is wanted
    }
}

impl<T: Config> XcmpMessageSource for Pallet<T> {
    fn take_outbound_messages(maximum_channels: usize) -> Vec<(Id, Vec<u8>)> {
        todo!()
        // Implement a proxy for the actual Self::XcmpSink
    }
}

impl<T: Config> ExecuteXcm<()> for Pallet<T> {
    fn execute_xcm_in_credit(origin: impl Into<MultiLocation>, message: Xcm<()>, weight_limit: u64, weight_credit: u64) -> Outcome {
        todo!()
        // Implement a proxy for the actual Self::DmpSource
        // This is usefull for:
        // * filtering out if response for the incoming xcm is wanted
    }
}

impl<T: Config> SendXcm for Pallet<T> {
    fn send_xcm(destination: impl Into<MultiLocation>, message: Xcm<()>) -> SendResult {
        todo!()
        // Implement the sending mechanism for the latest xcm-version in order to allow
        // others to use this pallet as a `SendXcm` object.
    }
}

impl<T: Config> WrapVersion for Pallet<T> {
    fn wrap_version(dest: &MultiLocation, xcm: impl Into<VersionedXcm>) -> Result<VersionedXcm, ()> {
        todo!()
        // Wrap an xcm into the supported xcm version of this pallet.
        // Polkadot uses this currently in their generic pallet that provides xcm
        // capabilities in order to map Multilocation-to-XcmVersion.
    }
}