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
use parity_scale_codec::Input;
use sp_std::vec::Vec;

/// An encoding & decoding trait for the purpose of meeting the
/// LiquidityPools General Message Passing Format
pub trait Codec: Sized {
	fn serialize(&self) -> Vec<u8>;
	fn deserialize<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error>;
}

#[cfg(any(test, feature = "std"))]
pub mod test_util {
	use parity_scale_codec::{Decode, Encode, Input, MaxEncodedLen};
	use scale_info::TypeInfo;

	use super::Codec;

	#[derive(Debug, Eq, PartialEq, Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
	pub struct Message;
	impl Codec for Message {
		fn serialize(&self) -> Vec<u8> {
			vec![0x42]
		}

		fn deserialize<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
			match input.read_byte()? {
				0x42 => Ok(Self),
				_ => Err("unsupported message".into()),
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
