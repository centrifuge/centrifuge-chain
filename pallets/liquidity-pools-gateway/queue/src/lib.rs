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

use std::fmt::Debug;

use cfg_traits::liquidity_pools::{MessageProcessor, MessageQueue as MessageQueueT};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
pub use pallet::*;
use parity_scale_codec::FullCodec;
use scale_info::TypeInfo;
use sp_runtime::traits::{AtLeast32BitUnsigned, EnsureAdd, EnsureAddAssign, One};

mod weights;

use weights::WeightInfo;

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
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ EnsureAdd
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		/// Type used for processing messages.
		type MessageProcessor: MessageProcessor<Message = Self::Message>;

		/// Weight information.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn message_nonce_store)]
	pub type MessageNonceStore<T: Config> = StorageValue<_, T::MessageNonce, ValueQuery>;

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
		#[pallet::weight(T::WeightInfo::process_message())]
		#[pallet::call_index(0)]
		pub fn process_message(origin: OriginFor<T>, nonce: T::MessageNonce) -> DispatchResult {
			ensure_signed(origin)?;

			let message = MessageQueue::<T>::take(nonce).ok_or(Error::<T>::MessageNotFound)?;

			match T::MessageProcessor::process(message.clone()) {
				Ok(_) => {
					Self::deposit_event(Event::<T>::MessageExecutionSuccess { nonce, message });

					Ok(())
				}
				Err(e) => {
					FailedMessageQueue::<T>::insert(nonce, (message.clone(), e.error));

					Self::deposit_event(Event::<T>::MessageExecutionFailure {
						nonce,
						message,
						error: e.error,
					});

					Ok(())
				}
			}
		}

		/// Convenience method for manually processing a failed message.
		#[pallet::weight(T::WeightInfo::process_failed_message())]
		#[pallet::call_index(1)]
		pub fn process_failed_message(
			origin: OriginFor<T>,
			nonce: T::MessageNonce,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let (message, _) =
				FailedMessageQueue::<T>::get(nonce).ok_or(Error::<T>::MessageNotFound)?;

			match T::MessageProcessor::process(message.clone()) {
				Ok(_) => {
					Self::deposit_event(Event::<T>::MessageExecutionSuccess { nonce, message });

					FailedMessageQueue::<T>::remove(nonce);

					Ok(())
				}
				Err(e) => {
					Self::deposit_event(Event::<T>::MessageExecutionFailure {
						nonce,
						message,
						error: e.error,
					});

					Ok(())
				}
			}
		}
	}

	impl<T: Config> Pallet<T> {
		fn service_message_queue(max_weight: Weight) -> Weight {
			let mut weight_used = Weight::zero();

			let mut processed_entries = Vec::new();

			for (nonce, message) in MessageQueue::<T>::iter() {
				processed_entries.push(nonce);

				let weight = match T::MessageProcessor::process(message.clone()) {
					Ok(post_info) => {
						Self::deposit_event(Event::MessageExecutionSuccess { nonce, message });

						post_info
							.actual_weight
							.expect("Message processing success already ensured")
							// Extra weight breakdown:
							//
							// 1 read for the message
							// 1 write for the event
							// 1 write for the message removal
							.saturating_add(T::DbWeight::get().reads_writes(1, 2))
					}
					Err(e) => {
						Self::deposit_event(Event::MessageExecutionFailure {
							nonce,
							message: message.clone(),
							error: e.error,
						});

						FailedMessageQueue::<T>::insert(nonce, (message, e.error));

						e.post_info
							.actual_weight
							.expect("Message processing success already ensured")
							// Extra weight breakdown:
							//
							// 1 read for the message
							// 1 write for the event
							// 1 write for the failed message
							// 1 write for the message removal
							.saturating_add(T::DbWeight::get().reads_writes(1, 3))
					}
				};

				weight_used = weight_used.saturating_add(weight);

				if weight_used.all_gte(max_weight) {
					break;
				}
			}

			for entry in processed_entries {
				MessageQueue::<T>::remove(entry);
			}

			weight_used
		}
	}

	impl<T: Config> MessageQueueT for Pallet<T> {
		type Message = T::Message;

		fn submit(message: Self::Message) -> DispatchResult {
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
