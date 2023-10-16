use cfg_primitives::{Balance, BlockNumber};
use cfg_traits::Seconds;
use sp_runtime::{DispatchResult, Storage};

use crate::{generic::runtime::Runtime, utils::accounts::Keyring};

/// Used by Env::pass() to determine how many blocks should be passed
#[derive(Clone)]
pub enum Blocks<T: Runtime> {
	/// Pass X blocks
	ByNumber(BlockNumber),

	/// Pass a number of blocks enough to emulate the given passage of time.
	/// i.e. choosing 1 sec would pass 1 block to emulate such change in the
	/// time.
	BySeconds(Seconds),

	/// Pass a number of block until find an event or reach the limit
	UntilEvent {
		event: T::RuntimeEventExt,
		limit: BlockNumber,
	},
}

/// Define an environment behavior
pub trait Env<T: Runtime> {
	/// Load the environment from a storage
	fn from_storage(storage: Storage) -> Self;

	/// Submit an extrinsic mutating the state
	fn submit(&mut self, who: Keyring, call: impl Into<T::RuntimeCall>) -> DispatchResult;

	/// Pass any number of blocks
	fn pass(&mut self, blocks: Blocks<T>);

	/// Allows to mutate the storage state through the closure
	fn state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R;

	/// Allows to read the storage state through the closure
	/// If storage is modified, it would not be applied.
	fn state<R>(&self, f: impl FnOnce() -> R) -> R;

	/// Check for an exact event introduced in the current block.
	/// Starting from last event introduced
	/// Returns an Option to unwrap it from the tests and have good panic
	/// message with the error test line
	fn check_event(&self, event: impl Into<T::RuntimeEventExt>) -> Option<()> {
		self.state(|| {
			let event = event.into();
			frame_system::Pallet::<T>::events()
				.into_iter()
				.rev()
				.find(|record| record.event == event)
				.map(|_| ())
		})
	}

	/// Find an event introduced in the current block
	/// Starting from last event introduced
	/// Returns an Option to unwrap it from the tests and have good panic
	/// message with the error test line
	fn find_event<E, R>(&self, f: impl Fn(E) -> Option<R>) -> Option<R>
	where
		T::RuntimeEventExt: TryInto<E>,
	{
		self.state(|| {
			frame_system::Pallet::<T>::events()
				.into_iter()
				.rev()
				.map(|record| record.event.try_into().ok())
				.find_map(|event| event.map(|e| f(e)))
				.flatten()
		})
	}

	/// Retrieve the fees used in the last submit call
	fn last_fee(&self) -> Balance {
		self.find_event(|e| match e {
			pallet_transaction_payment::Event::TransactionFeePaid { actual_fee, .. } => {
				Some(actual_fee)
			}
			_ => None,
		})
		.expect("Expected transaction")
	}
}
