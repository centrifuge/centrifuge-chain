use frame_support::dispatch::DispatchResult;

/// The trait required for processing outgoing connectors messages.
pub trait OutboundQueue {
	/// The sender type of the outgoing message.
	type Sender;

	/// The connector message enum.
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
