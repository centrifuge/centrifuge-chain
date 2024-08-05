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
	fn serialize(&self) -> Vec<u8>;
	fn deserialize(input: &[u8]) -> Result<Self, DispatchError>;
}

#[cfg(any(test, feature = "std"))]
pub mod test_util {
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
	use scale_info::TypeInfo;

	use super::*;

	#[derive(Default, Debug, Eq, PartialEq, Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct Message;
	impl LPEncoding for Message {
		fn serialize(&self) -> Vec<u8> {
			vec![0x42]
		}

		fn deserialize(input: &[u8]) -> Result<Self, DispatchError> {
			match input.first() {
				Some(0x42) => Ok(Self),
				Some(_) => Err("unsupported message".into()),
				None => Err("empty message".into()),
			}
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
