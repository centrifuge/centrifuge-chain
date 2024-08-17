use frame_support::{dispatch::DispatchResult, weights::Weight};

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

	/// Max weight that processing a message can take
	fn max_processing_weight(msg: &Self::Message) -> Weight;
}
