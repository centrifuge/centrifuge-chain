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

use frame_support::dispatch::DispatchResult;

///! Traits that stand in relation to the xcm-chain-adapter.

/// A sink that provides functionality for sending XCM.
///
/// This trait is abstract over the used transport mechanism (e.g. XCMP, UMP).
/// Instead, the implementor must take care of locating the correct consensus-system from
/// the provided information and use an appropriate channel then
///
/// The trait is meant to be abstract over a specific version of XCM and hence provides
/// associated types to wire in the actual types of the specific version.
///
/// E.g. One could choose to implement for only XCM-v0
/// ```
/// use xcm::v0::{Xcm, MultiLocation};
/// use frame_support::dispatch::DispatchResult;
/// use pallet_xcm_adapter::traits::XcmSink;
///
/// struct Sink;
///
/// impl XcmSink for Sink {
/// 	type Xcm = Xcm<()>;
/// 	type Receiver = MultiLocation;
///
/// 	fn send(msg: Self::Xcm, recv: Self::Receiver) -> DispatchResult {
/// 		todo!("Format message to transport protocol specific format and send via cumulus-parachain-system.");
/// 	}
/// }
/// ```
///
/// Or one could choose to implement for opaque XCM that includes all versions
///
/// ```
/// use xcm::{opaque::{VersionedXcm}, VersionedMultiLocation};
/// use frame_support::dispatch::DispatchResult;
/// use pallet_xcm_adapter::traits::XcmSink;
///
/// struct Sink;
///
/// impl XcmSink for Sink {
/// 	type Xcm = VersionedXcm;
/// 	type Receiver = VersionedMultiLocation;
///
/// 	fn send(msg: Self::Xcm, recv: Self::Receiver) -> DispatchResult {
/// 		todo!("Format message to transport protocol specific format and send via cumulus-parachain-system.");
/// 	}
/// }
/// ```
pub trait XcmSink {
	type Xcm;
	type Receiver;

	fn send(recv: Self::Receiver, msg: Self::Xcm) -> DispatchResult;
}

/// A handler that is able to receive responses from previously sent XCMs.
///
/// The trait is abstract over XCM. Implementations must decide how to translate from
/// the XCM specific objects to their own `Response` and `Error` types.
pub trait XcmResponseHandler {
	type Response;
	type Error;

	fn handle_response(resp: Self::Response);

	fn handle_error(err: Self::Error);
}

/// A router that decides wether something should go into the UMP-Queue or into the
/// XCMP-queue.
///
/// Implementors if this trait might wanna decide to alter the incoming receiving type to
/// a Multilocation of their will.
pub trait XcmRouter {
	type Xcm;
	type Receiver;

	fn route(recv: Self::Receiver, msg: Self::Xcm) -> Destination<Self::Receiver, Self::Xcm>;
}

/// An enum that defines which destination a XCM is routed to.
pub enum Destination<Xcm, Recv> {
	/// The XCM `Xcm` will be routed via an UMP channel to the receiver `Recv`
	Parent(Recv, Xcm),
	/// The XCM `Xcm` will be routed via an XCMP channel to the receiver `Recv`
	Sibling(Recv, Xcm),
}
