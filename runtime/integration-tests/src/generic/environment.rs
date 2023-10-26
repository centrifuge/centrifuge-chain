use cfg_primitives::{Address, Balance, BlockNumber, Index};
use cfg_traits::{IntoSeconds, Seconds};
use codec::Encode;
use sp_runtime::{
	generic::{Era, SignedPayload},
	traits::{Block, Extrinsic},
	DispatchError, DispatchResult, MultiSignature, Storage,
};

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

	/// Submit an extrinsic mutating the state instantly and returning the
	/// consumed fee
	fn submit_now(
		&mut self,
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
	) -> Result<Balance, DispatchError>;

	/// Submit an extrinsic mutating the state when the block is finalized
	fn submit_later(&mut self, who: Keyring, call: impl Into<T::RuntimeCallExt>) -> DispatchResult;

	/// Pass any number of blocks
	fn pass(&mut self, blocks: Blocks<T>) {
		let (next, end_block) = self.state(|| {
			let next = frame_system::Pallet::<T>::block_number() + 1;

			let end_block = next
				+ match blocks {
					Blocks::ByNumber(n) => n,
					Blocks::BySeconds(secs) => {
						let n = secs / pallet_aura::Pallet::<T>::slot_duration().into_seconds();
						if n % pallet_aura::Pallet::<T>::slot_duration() != 0 {
							n as BlockNumber + 1
						} else {
							n as BlockNumber
						}
					}
					Blocks::UntilEvent { limit, .. } => limit,
				};

			(next, end_block)
		});

		for i in next..end_block {
			self.__priv_build_block(i);

			if let Blocks::UntilEvent { event, .. } = blocks.clone() {
				if self.check_event(event).is_some() {
					break;
				}
			}
		}
	}

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
		);

		let raw_payload = SignedPayload::new(runtime_call.clone(), signed_extra.clone()).unwrap();
		let signature =
			MultiSignature::Sr25519(raw_payload.using_encoded(|payload| who.sign(payload)));

		let multi_address = (Address::Id(who.to_account_id()), signature, signed_extra);

		<T::Block as Block>::Extrinsic::new(runtime_call, Some(multi_address)).unwrap()
	}
}
