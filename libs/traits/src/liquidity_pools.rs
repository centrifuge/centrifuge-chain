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

/// An encoding & decoding trait for the purpose of meeting the
/// LiquidityPools General Message Passing Format
pub trait LPEncoding: Sized {
	const MAX_PACKED_MESSAGES: u32;

	fn serialize(&self) -> Vec<u8>;
	fn deserialize(input: &[u8]) -> Result<Self, DispatchError>;

	/// Compose this message with a new one
	fn pack(&self, other: Self) -> Result<Self, DispatchError>;

	/// Decompose the message into a list of messages
	/// If the message is not decomposable, it returns the own message.
	fn unpack(&self) -> Vec<Self>;

	/// Creates an empty message.
	/// It's the identity message for composing messages
	fn empty() -> Self;
}

#[cfg(any(test, feature = "std"))]
pub mod test_util {
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
	use scale_info::TypeInfo;

	use super::*;

	pub const DECODING_ERR_MSG: &str = "decoding message error";
	pub const EMPTY_ERR_MSG: &str = "empty message error error";

	#[derive(Default, Debug, Eq, PartialEq, Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct Message;
	impl LPEncoding for Message {
		const MAX_PACKED_MESSAGES: u32 = 1;

		fn serialize(&self) -> Vec<u8> {
			vec![0x42]
		}

		fn deserialize(input: &[u8]) -> Result<Self, DispatchError> {
			match input.first() {
				Some(0x42) => Ok(Self),
				Some(_) => Err(DECODING_ERR_MSG.into()),
				None => Err(EMPTY_ERR_MSG.into()),
			}
		}

		fn pack(&self, _: Self) -> Result<Self, DispatchError> {
			unimplemented!()
		}

		fn unpack(&self) -> Vec<Self> {
			vec![Self]
		}

		fn empty() -> Self {
			unimplemented!()
		}
	}
}

/// The trait required for sending outbound messages.
pub trait Router {
	/// The sender type of the outbound message.
	type Sender;

	/// Initialize the router.
	fn init(&self) -> DispatchResult;

	/// Send the message to the router's destination.
	fn send(&self, sender: Self::Sender, message: Vec<u8>) -> DispatchResultWithPostInfo;
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
