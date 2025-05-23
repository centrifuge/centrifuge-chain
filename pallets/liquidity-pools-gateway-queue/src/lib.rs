// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
#![cfg_attr(not(feature = "std"), no_std)]

use core::fmt::Debug;

use cfg_traits::liquidity_pools::{MessageProcessor, MessageQueue as MessageQueueT};
use frame_support::{dispatch::PostDispatchInfo, pallet_prelude::*};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use parity_scale_codec::FullCodec;
use scale_info::TypeInfo;
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::traits::{EnsureAddAssign, One};

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The message type.
		type Message: Clone + Debug + PartialEq + MaxEncodedLen + TypeInfo + FullCodec;

		/// Type used for message identification.
		type MessageNonce: Parameter
			+ Member
			+ BaseArithmetic
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		/// Type used for processing messages.
		type MessageProcessor: MessageProcessor<Message = Self::Message>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn message_nonce_store)]
	pub type MessageNonceStore<T: Config> = StorageValue<_, T::MessageNonce, ValueQuery>;

	/// Storage that is used for keeping track of the last nonce that was
	/// processed.
	#[pallet::storage]
	#[pallet::getter(fn last_processed_nonce)]
	pub type LastProcessedNonce<T: Config> = StorageValue<_, T::MessageNonce, ValueQuery>;

	/// Storage for messages that will be processed during the `on_idle` hook.
	#[pallet::storage]
	#[pallet::getter(fn message_queue)]
	pub type MessageQueue<T: Config> = StorageMap<_, Blake2_128Concat, T::MessageNonce, T::Message>;

	/// Storage for messages that failed during processing.
	#[pallet::storage]
	#[pallet::getter(fn failed_message_queue)]
	pub type FailedMessageQueue<T: Config> =
		StorageMap<_, Blake2_128Concat, T::MessageNonce, (T::Message, DispatchError)>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A message was submitted.
		MessageSubmitted {
			nonce: T::MessageNonce,
			message: T::Message,
		},

		/// Message execution success.
		MessageExecutionSuccess {
			nonce: T::MessageNonce,
			message: T::Message,
		},

		/// Message execution failure.
		MessageExecutionFailure {
			nonce: T::MessageNonce,
			message: T::Message,
			error: DispatchError,
		},

		/// Maximum number of messages was reached.
		MaxNumberOfMessagesReached {
			last_processed_nonce: T::MessageNonce,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Message not found.
		MessageNotFound,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_idle(_now: BlockNumberFor<T>, max_weight: Weight) -> Weight {
			Self::service_message_queue(max_weight)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Convenience method for manually processing a message.
		///
		/// If the execution fails, the message gets moved to the
		/// `FailedMessageQueue` storage.
		///
		/// NOTES:
		///   - this extrinsic does not error out during message processing
		/// to ensure that any storage changes (i.e. to the message queues)
		/// are not reverted.
		///   - an extra defensive weight is added in order to cover the weight
		/// used when processing the message.
		#[pallet::weight(MessageQueue::<T>::get(nonce)
            .map(|msg| T::MessageProcessor::max_processing_weight(&msg))
            .unwrap_or(T::DbWeight::get().reads(1)))]
		#[pallet::call_index(0)]
		pub fn process_message(
			origin: OriginFor<T>,
			nonce: T::MessageNonce,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			let message = MessageQueue::<T>::take(nonce).ok_or(Error::<T>::MessageNotFound)?;

			let (result, mut weight) =
				Self::process_message_and_deposit_event(nonce, message.clone());

			if let Err(e) = result {
				FailedMessageQueue::<T>::insert(nonce, (message, e));
				weight.saturating_accrue(T::DbWeight::get().writes(1));
			}

			// Add write from MessageQueue::take
			Ok(PostDispatchInfo::from(Some(
				weight.saturating_add(T::DbWeight::get().writes(1)),
			)))
		}

		/// Convenience method for manually processing a failed message.
		///
		/// If the execution is successful, the message gets removed from the
		/// `FailedMessageQueue` storage.
		///
		/// NOTES:
		///   - this extrinsic does not error out during message processing
		/// to ensure that any storage changes (i.e. to the message queues)
		/// are not reverted.
		///   - an extra defensive weight is added in order to cover the weight
		/// used when processing the message.
		#[pallet::weight(FailedMessageQueue::<T>::get(nonce)
            .map(|(msg, _)| T::MessageProcessor::max_processing_weight(&msg))
            .unwrap_or(T::DbWeight::get().reads(1)))]
		#[pallet::call_index(1)]
		pub fn process_failed_message(
			origin: OriginFor<T>,
			nonce: T::MessageNonce,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			let (message, _) =
				FailedMessageQueue::<T>::get(nonce).ok_or(Error::<T>::MessageNotFound)?;

			let (result, mut weight) = Self::process_message_and_deposit_event(nonce, message);

			if result.is_ok() {
				FailedMessageQueue::<T>::remove(nonce);
				weight.saturating_accrue(T::DbWeight::get().writes(1));
			}

			// Add read from FailedMessageQueue
			Ok(PostDispatchInfo::from(Some(
				weight.saturating_add(T::DbWeight::get().reads(1)),
			)))
		}
	}

	impl<T: Config> Pallet<T> {
		fn process_message_and_deposit_event(
			nonce: T::MessageNonce,
			message: T::Message,
		) -> (DispatchResult, Weight) {
			match T::MessageProcessor::process(message.clone()) {
				(Ok(()), weight) => {
					Self::deposit_event(Event::<T>::MessageExecutionSuccess { nonce, message });

					(Ok(()), weight)
				}
				(Err(error), weight) => {
					Self::deposit_event(Event::<T>::MessageExecutionFailure {
						nonce,
						message,
						error,
					});

					(Err(error), weight)
				}
			}
		}

		fn service_message_queue(max_weight: Weight) -> Weight {
			let mut last_processed_nonce = LastProcessedNonce::<T>::get();

			// 1 read for the last processed nonce
			let mut weight_used = T::DbWeight::get().reads(1);

			loop {
				if last_processed_nonce.ensure_add_assign(One::one()).is_err() {
					Self::deposit_event(Event::<T>::MaxNumberOfMessagesReached {
						last_processed_nonce,
					});

					break;
				}

				// 1 read for the nonce
				weight_used.saturating_accrue(T::DbWeight::get().reads(1));

				if last_processed_nonce > MessageNonceStore::<T>::get() {
					break;
				}

				// 1 read for the message
				weight_used.saturating_accrue(T::DbWeight::get().reads(1));

				let message = match MessageQueue::<T>::get(last_processed_nonce) {
					Some(msg) => msg,
					// No message found at this nonce, we can skip it.
					None => {
						LastProcessedNonce::<T>::set(last_processed_nonce);

						// 1 write for setting the last processed nonce
						weight_used.saturating_accrue(T::DbWeight::get().writes(1));

						continue;
					}
				};

				let remaining_weight = max_weight
					.saturating_sub(weight_used)
					.saturating_sub(T::DbWeight::get().reads(1));
				let next_weight = T::MessageProcessor::max_processing_weight(&message);

				// We ensure we have still capacity in the block before processing the message
				if remaining_weight.any_lt(next_weight) {
					break;
				}

				let processing_weight = match Self::process_message_and_deposit_event(
					last_processed_nonce,
					message.clone(),
				) {
					(Ok(()), weight) => weight,
					(Err(e), weight) => {
						FailedMessageQueue::<T>::insert(last_processed_nonce, (message, e));

						// 1 write for the failed message
						weight.saturating_add(T::DbWeight::get().writes(1))
					}
				};

				weight_used.saturating_accrue(processing_weight);

				MessageQueue::<T>::remove(last_processed_nonce);

				LastProcessedNonce::<T>::set(last_processed_nonce);

				// 1 write for removing the message
				// 1 write for setting the last processed nonce
				weight_used.saturating_accrue(T::DbWeight::get().writes(2));
			}

			weight_used
		}
	}

	impl<T: Config> MessageQueueT for Pallet<T> {
		type Message = T::Message;

		fn queue(message: Self::Message) -> DispatchResult {
			let nonce = <MessageNonceStore<T>>::try_mutate(|n| {
				n.ensure_add_assign(T::MessageNonce::one())?;
				Ok::<T::MessageNonce, DispatchError>(*n)
			})?;

			MessageQueue::<T>::insert(nonce, message.clone());

			Self::deposit_event(Event::MessageSubmitted { nonce, message });

			Ok(())
		}
	}
}
