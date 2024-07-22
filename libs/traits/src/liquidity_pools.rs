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

use frame_support::dispatch::{DispatchResult, DispatchResultWithPostInfo};
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

/// An encoding & decoding trait for the purpose of meeting the
/// LiquidityPools General Message Passing Format
pub trait LPEncoding: Sized {
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

	#[derive(Debug, Eq, PartialEq, Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
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

/// The trait required for processing outbound messages.
pub trait OutboundQueue {
	/// The sender type of the outgoing message.
	type Sender;

	/// The destination this message should go to.
	type Destination;

	/// The message type that is processed.
	type Message;

	/// Submit a message to the outbound queue.
	fn submit(
		sender: Self::Sender,
		destination: Self::Destination,
		msg: Self::Message,
	) -> DispatchResult;
}

/// The trait required for processing incoming messages.
pub trait InboundQueue {
	/// The sender type of the incoming message.
	type Sender;

	/// The liquidityPools message type.
	type Message;

	/// Submit a message to the inbound queue.
	fn submit(sender: Self::Sender, msg: Self::Message) -> DispatchResult;
}
