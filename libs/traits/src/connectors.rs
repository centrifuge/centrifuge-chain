// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

use codec::Input;
use frame_support::dispatch::DispatchResult;
use sp_std::vec::Vec;

/// An encoding & decoding trait for the purpose of meeting the
/// Connectors General Message Passing Format
pub trait Codec: Sized {
	fn serialize(&self) -> Vec<u8>;
	fn deserialize<I: Input>(input: &mut I) -> Result<Self, codec::Error>;
}

/// The trait required for sending outbound messages.
pub trait Router {
	/// The sender type of the outbound message.
	type Sender;

	/// The outbound message type.
	type Message;

	/// Send the message to the router's destination.
	fn send(&self, sender: Self::Sender, message: Self::Message) -> DispatchResult;
}

/// The trait required for processing outbound connectors messages.
pub trait OutboundQueue {
	/// The sender type of the outgoing message.
	type Sender;

	/// The message type that is processed.
	type Message;

	/// The destination this message should go to.
	type Destination;

	/// Submit a message to the outbound queue.
	fn submit(
		destination: Self::Destination,
		sender: Self::Sender,
		msg: Self::Message,
	) -> DispatchResult;
}

/// The trait required for processing incoming connectors messages.
pub trait InboundQueue {
	/// The sender type of the incoming message.
	type Sender;

	/// The connector message enum.
	type Message;

	/// Submit a message to the inbound queue.
	fn submit(sender: Self::Sender, msg: Self::Message) -> DispatchResult;
}
