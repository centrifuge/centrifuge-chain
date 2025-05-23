// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
//! # Liquidity Pools Gateway Pallet.
//!
//! The Liquidity Pools Gateway pallet is the main handler of incoming and
//! outgoing Liquidity Pools messages.
//!
//! For incoming messages it validates the `Domain` and address of the sender,
//! and, upon successful validation, it sends the message to the `InboundQueue`
//! for further processing. The pallet that implements the `InboundQueue` is the
//! Liquidity Pools pallet.
//!
//! For outgoing messages it's using a queue which gets serviced when a block
//! gets finalized. Each message in the `OutboundMessageQueue` has a `Domain`
//! assigned to it, and that `Domain` should have a corresponding `DomainRouter`
//! which should be set prior to sending the message.
#![cfg_attr(not(feature = "std"), no_std)]

use core::fmt::Debug;

use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{
	InboundMessageHandler, LpMessageBatch, LpMessageProof, LpMessageRecovery, LpMessageSerializer,
	MessageHash, MessageProcessor, MessageQueue, MessageReceiver, MessageSender,
	OutboundMessageHandler, RouterProvider,
};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::*,
	storage::{with_transaction, TransactionOutcome},
};
use frame_system::pallet_prelude::{ensure_signed, OriginFor};
use message::GatewayMessage;
use orml_traits::GetByKey;
pub use pallet::*;
use parity_scale_codec::FullCodec;
use sp_arithmetic::traits::{BaseArithmetic, EnsureAddAssign, One};
use sp_std::convert::TryInto;

use crate::{
	message_processing::{InboundEntry, ProofEntry},
	weights::WeightInfo,
};

pub mod message;

pub mod weights;

mod message_processing;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(3);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The AdminOrigin ensures that some calls can only be performed by
		/// admins.
		type AdminOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// The Liquidity Pools message type.
		type Message: LpMessageSerializer
			+ LpMessageBatch
			+ LpMessageProof
			+ LpMessageRecovery
			+ Clone
			+ Debug
			+ PartialEq
			+ Eq
			+ MaxEncodedLen
			+ TypeInfo
			+ FullCodec;

		/// The target of the messages coming from this chain
		type MessageSender: MessageSender<
			Middleware = Self::RouterId,
			Origin = DomainAddress,
			Message = Self::Message,
		>;

		/// An identification of a router
		type RouterId: Parameter + MaxEncodedLen + Into<Domain>;

		/// The type that provides the router available for a domain.
		type RouterProvider: RouterProvider<Domain, RouterId = Self::RouterId>;

		/// The type that processes inbound messages.
		type InboundMessageHandler: InboundMessageHandler<Sender = Domain, Message = Self::Message>;

		type WeightInfo: WeightInfo;

		/// Maximum size of an incoming message.
		#[pallet::constant]
		type MaxIncomingMessageSize: Get<u32>;

		/// The sender account that will be used in the OutboundQueue
		/// implementation.
		#[pallet::constant]
		type Sender: Get<DomainAddress>;

		/// Type used for queueing messages.
		type MessageQueue: MessageQueue<Message = GatewayMessage<Self::Message, Self::RouterId>>;

		/// Maximum number of routers allowed for a domain.
		#[pallet::constant]
		type MaxRouterCount: Get<u32>;

		/// Type for identifying sessions of inbound routers.
		type SessionId: Parameter + Member + BaseArithmetic + Default + Copy + MaxEncodedLen;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The routers for a given domain were set.
		RoutersSet {
			router_ids: BoundedVec<T::RouterId, T::MaxRouterCount>,
			session_id: T::SessionId,
		},

		/// The domain hook address was initialized or updated.
		DomainHookAddressSet {
			domain: Domain,
			hook_address: [u8; 20],
		},

		/// An inbound message was processed.
		InboundMessageProcessed {
			domain: Domain,
			message_hash: MessageHash,
			router_id: T::RouterId,
		},

		/// An inbound message proof was processed.
		InboundProofProcessed {
			domain: Domain,
			message_hash: MessageHash,
			router_id: T::RouterId,
		},

		/// An inbound message was executed.
		InboundMessageExecuted {
			domain: Domain,
			message_hash: MessageHash,
		},

		/// An outbound message was sent.
		OutboundMessageSent {
			domain_address: DomainAddress,
			message_hash: MessageHash,
			router_id: T::RouterId,
		},

		/// Message recovery was executed.
		MessageRecoveryExecuted {
			message_hash: MessageHash,
			router_id: T::RouterId,
		},

		/// Message recovery was initiated.
		MessageRecoveryInitiated {
			domain: Domain,
			message_hash: MessageHash,
			recovery_router: [u8; 32],
			messaging_router: T::RouterId,
		},

		/// Message recovery was disputed.
		MessageRecoveryDisputed {
			domain: Domain,
			message_hash: MessageHash,
			recovery_router: [u8; 32],
			messaging_router: T::RouterId,
		},
	}

	/// Storage for routers.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	#[pallet::getter(fn routers)]
	pub type Routers<T: Config> =
		StorageValue<_, BoundedVec<T::RouterId, T::MaxRouterCount>, ValueQuery>;

	/// Stores the hook address of a domain required for particular LP messages.
	///
	/// Lifetime: Indefinitely.
	///
	/// NOTE: Must only be changeable via `AdminOrigin`.
	#[pallet::storage]
	pub type DomainHookAddress<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, [u8; 20], OptionQuery>;

	/// Stores a batch message, not ready yet to be enqueued.
	/// Lifetime handled by `start_batch_message()` and `end_batch_message()`
	/// extrinsics.
	#[pallet::storage]
	pub(crate) type PackedMessage<T: Config> =
		StorageMap<_, Blake2_128Concat, (T::AccountId, Domain), T::Message>;

	/// Storage for pending inbound messages.
	#[pallet::storage]
	#[pallet::getter(fn pending_inbound_entries)]
	pub type PendingInboundEntries<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		MessageHash,
		Blake2_128Concat,
		T::RouterId,
		InboundEntry<T>,
	>;

	/// Storage for inbound message session IDs.
	#[pallet::storage]
	pub type SessionIdStore<T: Config> = StorageValue<_, T::SessionId, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// The domain is not supported.
		DomainNotSupported,

		/// Emitted when you call `start_batch_messages()` but that was already
		/// called. You should finalize the message with `end_batch_messages()`
		MessagePackingAlreadyStarted,

		/// Emitted when you can `end_batch_message()` but the packing process
		/// was not started by `start_batch_message()`.
		MessagePackingNotStarted,

		/// Unknown router.
		UnknownRouter,

		/// Messaging router not found.
		MessagingRouterNotFound,

		/// The router that sent the message is not the first one.
		MessageExpectedFromFirstRouter,

		/// The router that sent the proof should not be the first one.
		ProofNotExpectedFromFirstRouter,

		/// A message was expected instead of a proof.
		ExpectedMessageType,

		/// A message proof was expected instead of a message.
		ExpectedMessageProofType,

		/// Pending inbound entry not found.
		PendingInboundEntryNotFound,

		/// Not enough routers are stored for a domain.
		NotEnoughRoutersForDomain,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the IDs of the routers that are used when receiving and sending
		/// messages.
		#[pallet::weight(T::WeightInfo::set_routers())]
		#[pallet::call_index(0)]
		pub fn set_routers(
			origin: OriginFor<T>,
			router_ids: BoundedVec<T::RouterId, T::MaxRouterCount>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			<Routers<T>>::set(router_ids.clone());

			let new_session_id = SessionIdStore::<T>::try_mutate(|n| {
				n.ensure_add_assign(One::one())?;

				Ok::<T::SessionId, DispatchError>(*n)
			})?;

			Self::deposit_event(Event::RoutersSet {
				router_ids,
				session_id: new_session_id,
			});

			Ok(())
		}

		/// Set the address of the domain hook
		///
		/// Can only be called by `AdminOrigin`.
		#[pallet::weight(T::WeightInfo::set_domain_hook_address())]
		#[pallet::call_index(8)]
		pub fn set_domain_hook_address(
			origin: OriginFor<T>,
			domain: Domain,
			hook_address: [u8; 20],
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			ensure!(domain != Domain::Centrifuge, Error::<T>::DomainNotSupported);
			DomainHookAddress::<T>::insert(domain, hook_address);

			Self::deposit_event(Event::DomainHookAddressSet {
				domain,
				hook_address,
			});

			Ok(())
		}

		/// Start packing messages in a single message instead of enqueue
		/// messages.
		/// The message will be enqueued once `end_batch_messages()` is called.
		#[pallet::weight(T::WeightInfo::start_batch_message())]
		#[pallet::call_index(9)]
		pub fn start_batch_message(origin: OriginFor<T>, destination: Domain) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			PackedMessage::<T>::mutate((&sender, &destination), |msg| match msg {
				Some(_) => Err(Error::<T>::MessagePackingAlreadyStarted.into()),
				None => {
					*msg = Some(T::Message::empty());
					Ok(())
				}
			})
		}

		/// End packing messages.
		/// If exists any batch message it will be enqueued.
		/// Empty batches are no-op
		#[pallet::weight(T::WeightInfo::end_batch_message())]
		#[pallet::call_index(10)]
		pub fn end_batch_message(origin: OriginFor<T>, destination: Domain) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			match PackedMessage::<T>::take((&sender, &destination)) {
				Some(msg) if msg.submessages().is_empty() => Ok(()), //No-op
				Some(message) => Self::queue_outbound_message(destination, message),
				None => Err(Error::<T>::MessagePackingNotStarted.into()),
			}
		}

		/// Manually increase the proof count for a particular message and
		/// executes it if the required count is reached.
		///
		/// Can only be called by `AdminOrigin`.
		#[pallet::weight(T::WeightInfo::execute_message_recovery())]
		#[pallet::call_index(11)]
		pub fn execute_message_recovery(
			origin: OriginFor<T>,
			domain: Domain,
			message_hash: MessageHash,
			router_id: T::RouterId,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			let router_ids = Self::get_router_ids_for_domain(domain)?;

			ensure!(
				router_ids.iter().any(|x| x == &router_id),
				Error::<T>::UnknownRouter
			);
			// Message recovery shouldn't be supported for setups that have less than 2
			// routers since no proofs are required in that case.
			ensure!(router_ids.len() > 1, Error::<T>::NotEnoughRoutersForDomain);

			let session_id = SessionIdStore::<T>::get();

			PendingInboundEntries::<T>::try_mutate(
				message_hash,
				router_id.clone(),
				|storage_entry| match storage_entry {
					Some(stored_inbound_entry) => {
						stored_inbound_entry.increment_proof_count(session_id)
					}
					None => {
						*storage_entry = Some(InboundEntry::<T>::Proof(ProofEntry {
							session_id,
							current_count: 1,
						}));

						Ok::<(), DispatchError>(())
					}
				},
			)?;

			let expected_proof_count = Self::get_expected_proof_count(&router_ids)?;

			Self::execute_if_requirements_are_met(
				message_hash,
				&router_ids,
				session_id,
				expected_proof_count,
				domain,
			)?;

			Self::deposit_event(Event::<T>::MessageRecoveryExecuted {
				message_hash,
				router_id,
			});

			Ok(())
		}

		/// Sends a message that initiates a message recovery using the
		/// messaging router.
		///
		/// Can only be called by `AdminOrigin`.
		#[pallet::weight(T::WeightInfo::initiate_message_recovery())]
		#[pallet::call_index(12)]
		pub fn initiate_message_recovery(
			origin: OriginFor<T>,
			message_hash: MessageHash,
			recovery_router: [u8; 32],
			messaging_router: T::RouterId,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			let domain = messaging_router.clone().into();

			let message = T::Message::initiate_recovery_message(message_hash, recovery_router);

			Self::send_recovery_message(domain, message, messaging_router.clone())?;

			Self::deposit_event(Event::<T>::MessageRecoveryInitiated {
				domain,
				message_hash,
				recovery_router,
				messaging_router,
			});

			Ok(())
		}

		/// Sends a message that disputes a message recovery using the
		/// messaging router.
		///
		/// Can only be called by `AdminOrigin`.
		#[pallet::weight(T::WeightInfo::dispute_message_recovery())]
		#[pallet::call_index(13)]
		pub fn dispute_message_recovery(
			origin: OriginFor<T>,
			message_hash: MessageHash,
			recovery_router: [u8; 32],
			messaging_router: T::RouterId,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			let domain = messaging_router.clone().into();

			let message = T::Message::dispute_recovery_message(message_hash, recovery_router);

			Self::send_recovery_message(domain, message, messaging_router.clone())?;

			Self::deposit_event(Event::<T>::MessageRecoveryDisputed {
				domain,
				message_hash,
				recovery_router,
				messaging_router,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn send_recovery_message(
			domain: Domain,
			message: T::Message,
			messaging_router: T::RouterId,
		) -> DispatchResult {
			let router_ids = Self::get_router_ids_for_domain(domain)?;

			ensure!(
				router_ids.iter().any(|x| x == &messaging_router),
				Error::<T>::MessagingRouterNotFound
			);

			T::MessageSender::send(messaging_router, T::Sender::get(), message)
		}
	}

	impl<T: Config> OutboundMessageHandler for Pallet<T> {
		type Destination = Domain;
		type Message = T::Message;
		type Sender = T::AccountId;

		fn handle(
			from: Self::Sender,
			destination: Self::Destination,
			message: Self::Message,
		) -> DispatchResult {
			ensure!(
				destination != Domain::Centrifuge,
				Error::<T>::DomainNotSupported
			);

			PackedMessage::<T>::mutate((&from, destination), |batch| match batch {
				Some(batch) => batch.pack_with(message),
				None => Self::queue_outbound_message(destination, message),
			})
		}
	}

	impl<T: Config> GetByKey<Domain, Option<[u8; 20]>> for Pallet<T> {
		fn get(domain: &Domain) -> Option<[u8; 20]> {
			DomainHookAddress::<T>::get(domain)
		}
	}

	impl<T: Config> MessageProcessor for Pallet<T> {
		type Message = GatewayMessage<T::Message, T::RouterId>;

		fn process(msg: Self::Message) -> (DispatchResult, Weight) {
			// The #[transactional] macro only works for functions that return a
			// `DispatchResult` therefore, we need to manually add this here.
			let res = with_transaction(|| {
				let res = match msg {
					GatewayMessage::Inbound {
						domain,
						message,
						router_id,
					} => Self::process_inbound_message(domain, message, router_id),
					GatewayMessage::Outbound { message, router_id } => {
						T::MessageSender::send(router_id, T::Sender::get(), message)
					}
				};

				if res.is_ok() {
					TransactionOutcome::Commit(res)
				} else {
					TransactionOutcome::Rollback(res)
				}
			});

			(res, LP_DEFENSIVE_WEIGHT)
		}

		/// Returns the maximum weight for processing one message.
		fn max_processing_weight(_: &Self::Message) -> Weight {
			LP_DEFENSIVE_WEIGHT
		}
	}

	impl<T: Config> MessageReceiver for Pallet<T> {
		type Message = T::Message;
		type Middleware = T::RouterId;
		type Origin = Domain;

		fn receive(router_id: T::RouterId, domain: Domain, message: T::Message) -> DispatchResult {
			let gateway_message = GatewayMessage::<T::Message, T::RouterId>::Inbound {
				domain,
				message,
				router_id,
			};

			T::MessageQueue::queue(gateway_message)
		}
	}
}
