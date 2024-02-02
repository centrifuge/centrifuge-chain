use cfg_primitives::{Address, Balance, BlockNumber, Index};
use cfg_traits::{IntoSeconds, Seconds};
use parity_scale_codec::Encode;
use sp_runtime::{
	generic::{Era, SignedPayload},
	traits::{Block, Extrinsic},
	DispatchError, MultiSignature, Storage,
};
use sp_std::ops::Range;

use crate::{generic::config::Runtime, utils::accounts::Keyring};

/// Used by Env::pass() to determine how many blocks should be passed
#[derive(Clone)]
pub enum Blocks<Event> {
	/// Pass X blocks
	ByNumber(BlockNumber),

	/// Pass a number of blocks enough to emulate the given passage of time.
	/// i.e. choosing 1 sec would pass 1 block to emulate such change in the
	/// time.
	/// See the test below for an example
	BySeconds(Seconds),

	/// Pass a number of block until find an event or reach the limit
	UntilEvent { event: Event, limit: BlockNumber },

	/// Jumps to a block in the future to reach the requested time.
	/// Only one real block is created in the process.
	/// This can be used to emulate passing time during long periods
	/// computationally very fast.
	/// (i.e. years)
	JumpBySeconds(Seconds),
}

impl<Event> Blocks<Event> {
	fn range_for(&self, current: BlockNumber, slot_duration: Seconds) -> Range<BlockNumber> {
		let next = current + 1;
		let (from, to) = match self {
			Blocks::ByNumber(n) => (next, next + *n),
			Blocks::BySeconds(secs) => {
				let mut blocks = (secs / slot_duration) as BlockNumber;
				if secs % slot_duration != 0 {
					blocks += 1
				};
				(next, next + blocks)
			}
			Blocks::UntilEvent { limit, .. } => (next, next + *limit),
			Blocks::JumpBySeconds(secs) => {
				let mut blocks = (secs / slot_duration) as BlockNumber;
				if secs % slot_duration != 0 {
					blocks += 1
				};
				(next + blocks.saturating_sub(1), next + blocks)
			}
		};
		from..to
	}
}

/// Define an environment behavior
pub trait Env<T: Runtime>: Default {
	/// Load the environment from a parachain storage
	fn from_parachain_storage(parachain_storage: Storage) -> Self;

	/// Load the environment from a storage
	fn from_storage(
		relay_storage: Storage,
		parachain_storage: Storage,
		sibling_storage: Storage,
	) -> Self;

	/// Submit an extrinsic mutating the state instantly and returning the
	/// consumed fee
	fn submit_now(
		&mut self,
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
	) -> Result<Balance, DispatchError>;

	/// Submit an extrinsic mutating the state when the block is finalized
	fn submit_later(
		&mut self,
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
	) -> Result<(), Box<dyn std::error::Error>>;

	/// Pass any number of blocks
	fn pass(&mut self, blocks: Blocks<T::RuntimeEventExt>) {
		let (current, slot) = self.parachain_state(|| {
			(
				frame_system::Pallet::<T>::block_number(),
				pallet_aura::Pallet::<T>::slot_duration().into_seconds(),
			)
		});

		for i in blocks.range_for(current, slot) {
			self.__priv_build_block(i);

			if let Blocks::UntilEvent { event, .. } = blocks.clone() {
				if self.check_event(event).is_some() {
					break;
				}
			}
		}
	}

	/// Allows to mutate the relay storage state through the closure.
	fn relay_state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R;

	/// Allows to read the relay storage state through the closure.
	fn relay_state<R>(&self, f: impl FnOnce() -> R) -> R;

	/// Allows to mutate the parachain storage state through the closure.
	fn parachain_state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R;

	/// Allows to read the parachain storage state through the closure.
	fn parachain_state<R>(&self, f: impl FnOnce() -> R) -> R;

	/// Allows to mutate the sibling storage state through the closure.
	fn sibling_state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R;

	/// Allows to read the sibling storage state through the closure.
	fn sibling_state<R>(&self, f: impl FnOnce() -> R) -> R;

	/// Check for an exact event introduced in the current block.
	/// Starting from last event introduced
	/// Returns an Option to unwrap it from the tests and have good panic
	/// message with the error test line
	fn check_event(&self, event: impl Into<T::RuntimeEventExt>) -> Option<()> {
		self.parachain_state(|| {
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
		self.parachain_state(|| {
			frame_system::Pallet::<T>::events()
				.into_iter()
				.rev()
				.find_map(|record| record.event.try_into().map(|e| f(e)).ok())
				.flatten()
		})
	}

	fn __priv_build_block(&mut self, i: BlockNumber);
}

pub mod utils {
	use super::*;

	/// Creates an extrinsic, used mainly by the environment implementations.
	/// To create and submit an extrinsic, see `submit()`
	pub fn create_extrinsic<T: Runtime>(
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
		nonce: Index,
	) -> <T::Block as Block>::Extrinsic {
		let runtime_call = call.into();
		let signed_extra = (
			frame_system::CheckNonZeroSender::<T>::new(),
			frame_system::CheckSpecVersion::<T>::new(),
			frame_system::CheckTxVersion::<T>::new(),
			frame_system::CheckGenesis::<T>::new(),
			frame_system::CheckEra::<T>::from(Era::mortal(256, 0)),
			frame_system::CheckNonce::<T>::from(nonce),
			frame_system::CheckWeight::<T>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<T>::from(0),
			runtime_common::transfer_filter::PreBalanceTransferExtension::<T>::new(),
		);

		let raw_payload = SignedPayload::new(runtime_call.clone(), signed_extra.clone()).unwrap();
		let signature =
			MultiSignature::Sr25519(raw_payload.using_encoded(|payload| who.sign(payload)));

		let multi_address = (Address::Id(who.to_account_id()), signature, signed_extra);

		<T::Block as Block>::Extrinsic::new(runtime_call, Some(multi_address)).unwrap()
	}
}

mod tests {
	use super::*;
	struct MockEnv;

	const SLOT_DURATION: Seconds = 12;
	const EMPTY: [BlockNumber; 0] = [];

	fn blocks_from(current: BlockNumber, blocks: Blocks<()>) -> Vec<BlockNumber> {
		blocks
			.range_for(current, SLOT_DURATION)
			.into_iter()
			.collect()
	}

	#[test]
	fn by_seconds() {
		assert_eq!(blocks_from(0, Blocks::BySeconds(0)), EMPTY);
		assert_eq!(blocks_from(0, Blocks::BySeconds(1)), [1]);
		assert_eq!(blocks_from(0, Blocks::BySeconds(12)), [1]);
		assert_eq!(blocks_from(5, Blocks::BySeconds(0)), EMPTY);
		assert_eq!(blocks_from(5, Blocks::BySeconds(12)), [6]);
		assert_eq!(blocks_from(5, Blocks::BySeconds(60)), [6, 7, 8, 9, 10]);
	}

	#[test]
	fn by_seconds_fast() {
		assert_eq!(blocks_from(0, Blocks::JumpBySeconds(0)), EMPTY);
		assert_eq!(blocks_from(0, Blocks::JumpBySeconds(1)), [1]);
		assert_eq!(blocks_from(0, Blocks::JumpBySeconds(12)), [1]);
		assert_eq!(blocks_from(5, Blocks::JumpBySeconds(0)), EMPTY);
		assert_eq!(blocks_from(5, Blocks::JumpBySeconds(12)), [6]);
		assert_eq!(blocks_from(5, Blocks::JumpBySeconds(60)), [10]);
	}
}
