// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use frame_support::{
	dispatch::{DispatchResult, DispatchResultWithPostInfo},
	weights::Weight,
};
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

pub type Proof = [u8; 32];

/// An encoding & decoding trait for the purpose of meeting the
/// LiquidityPools General Message Passing Format
pub trait LPEncoding: Sized {
	fn serialize(&self) -> Vec<u8>;
	fn deserialize(input: &[u8]) -> Result<Self, DispatchError>;

	/// Extend this message with a new one
	fn pack_with(&mut self, other: Self) -> DispatchResult;

	/// Decompose the message into a list of messages
	/// If the message is not decomposable, it returns the own message.
	fn submessages(&self) -> Vec<Self>;

	/// Creates an empty message.
	/// It's the identity message for composing messages with pack_with
	fn empty() -> Self;

	fn get_message_proof(&self) -> Option<Proof>;
	fn to_message_proof(&self) -> Self;
}

/// The trait required for sending outbound messages.
pub trait Router {
	/// The sender type of the outbound message.
	type Sender;

	/// The router hash type.
	type Hash;

	/// Initialize the router.
	fn init(&self) -> DispatchResult;

	/// Send the message to the router's destination.
	fn send(&self, sender: Self::Sender, message: Vec<u8>) -> DispatchResultWithPostInfo;

	/// Generate a hash for this router.
	fn hash(&self) -> Self::Hash;
}

/// The trait required for queueing messages.
pub trait MessageQueue {
	/// The message type.
	type Message;

	/// Submit a message to the queue.
	fn submit(msg: Self::Message) -> DispatchResult;
}

/// The trait required for processing queued messages.
pub trait MessageProcessor {
	/// The message type.
	type Message;

	/// Process a message.
	fn process(msg: Self::Message) -> (DispatchResult, Weight);

	/// Process a message.
	fn max_processing_weight(msg: &Self::Message) -> Weight;
}

/// The trait required for handling outbound LP messages.
pub trait OutboundMessageHandler {
	/// The sender type of the outbound message.
	type Sender;

	/// The destination type of the outbound message.
	type Destination;

	/// The message type.
	type Message;

	/// Handle an outbound message.
	fn handle(
		sender: Self::Sender,
		destination: Self::Destination,
		msg: Self::Message,
	) -> DispatchResult;
}

/// The trait required for handling inbound LP messages.
pub trait InboundMessageHandler {
	/// The sender type of the inbound message.
	type Sender;

	/// The message type.
	type Message;

	/// Handle an inbound message.
	fn handle(sender: Self::Sender, msg: Self::Message) -> DispatchResult;
}
