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
	InboundMessageHandler, LPEncoding, MessageProcessor, MessageQueue, MessageReceiver,
	MessageSender, OutboundMessageHandler, RouterSupport,
};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::{ensure_signed, OriginFor};
use message::GatewayMessage;
use orml_traits::GetByKey;
pub use pallet::*;
use parity_scale_codec::{EncodeLike, FullCodec};
use sp_arithmetic::traits::{BaseArithmetic, EnsureSub, One};
use sp_runtime::traits::EnsureAddAssign;
use sp_std::{cmp::Ordering, convert::TryInto, vec::Vec};

use crate::weights::WeightInfo;

mod origin;
pub use origin::*;

pub mod message;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// Type used when storing inbound message information.
#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub enum InboundEntry<T: Config> {
	Message {
		domain_address: DomainAddress,
		message: T::Message,
		expected_proof_count: u32,
	},
	Proof {
		current_count: u32,
	},
}

/// Type used when processing inbound messages.
#[derive(Clone)]
pub struct InboundProcessingInfo<T: Config> {
	domain_address: DomainAddress,
	inbound_routers: BoundedVec<T::Hash, T::MaxRouterCount>,
	current_session_id: T::SessionId,
	expected_proof_count_per_message: u32,
}

#[frame_support::pallet]
pub mod pallet {
	use sp_arithmetic::traits::EnsureAdd;

	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::origin]
	pub type Origin = GatewayOrigin;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The origin type.
		type RuntimeOrigin: Into<Result<GatewayOrigin, <Self as frame_system::Config>::RuntimeOrigin>>
			+ From<GatewayOrigin>;

		/// The event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The LocalOrigin ensures that some calls can only be performed from a
		/// local context i.e. a different pallet.
		type LocalEVMOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = GatewayOrigin,
		>;

		/// The AdminOrigin ensures that some calls can only be performed by
		/// admins.
		type AdminOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// The Liquidity Pools message type.
		type Message: LPEncoding + Clone + Debug + PartialEq + MaxEncodedLen + TypeInfo + FullCodec;

		/// The target of of the messages comming from this chain
		type MessageSender: MessageSender<Middleware = Self::RouterId, Origin = DomainAddress>;

		/// An identification of a router
		type RouterId: RouterSupport<Domain> + Parameter;

		/// The type that processes inbound messages.
		type InboundMessageHandler: InboundMessageHandler<
			Sender = DomainAddress,
			Message = Self::Message,
		>;

		type WeightInfo: WeightInfo;

		/// Maximum size of an incoming message.
		#[pallet::constant]
		type MaxIncomingMessageSize: Get<u32>;

		/// The sender account that will be used in the OutboundQueue
		/// implementation.
		#[pallet::constant]
		type Sender: Get<DomainAddress>;

		/// Type used for queueing messages.
		type MessageQueue: MessageQueue<
			Message = GatewayMessage<Self::AccountId, Self::Message, Self::Hash>,
		>;

		/// Maximum number of routers allowed for a domain.
		#[pallet::constant]
		type MaxRouterCount: Get<u32>;

		/// Type for identifying sessions of inbound routers.
		type SessionId: Parameter
			+ Member
			+ BaseArithmetic
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An instance was added to a domain.
		InstanceAdded { instance: DomainAddress },

		/// An instance was removed from a domain.
		InstanceRemoved { instance: DomainAddress },

		/// The domain hook address was initialized or updated.
		DomainHookAddressSet {
			domain: Domain,
			hook_address: [u8; 20],
		},

		/// The outbound routers for a given domain were set.
		OutboundRoutersSet {
			domain: Domain,
			routers: BoundedVec<T::Router, T::MaxRouterCount>,
		},

		/// Inbound routers were set.
		InboundRoutersSet {
			domain: Domain,
			router_hashes: BoundedVec<T::Hash, T::MaxRouterCount>,
		},
	}

	/// Storage that contains a limited number of whitelisted instances of
	/// deployed liquidity pools for a particular domain.
	///
	/// This can only be modified by an admin.
	#[pallet::storage]
	#[pallet::getter(fn allowlist)]
	pub type Allowlist<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, Domain, Blake2_128Concat, DomainAddress, ()>;

	/// Stores the hook address of a domain required for particular LP messages.
	///
	/// Lifetime: Indefinitely.
	///
	/// NOTE: Must only be changeable via `AdminOrigin`.
	#[pallet::storage]
	pub type DomainHookAddress<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, [u8; 20], OptionQuery>;

	/// Stores a batch message, not ready yet to be enqueue.
	/// Lifetime handled by `start_batch_message()` and `end_batch_message()`
	/// extrinsics.
	#[pallet::storage]
	pub(crate) type PackedMessage<T: Config> =
		StorageMap<_, Blake2_128Concat, (T::AccountId, Domain), T::Message>;

	/// Storage for outbound routers.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	#[pallet::getter(fn routers)]
	pub type OutboundRouters<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, T::Router>;

	/// Storage for outbound routers specific for a domain.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	#[pallet::getter(fn outbound_domain_routers)]
	pub type OutboundDomainRouters<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, BoundedVec<T::Hash, T::MaxRouterCount>>;

	/// Storage for pending inbound messages.
	#[pallet::storage]
	#[pallet::getter(fn pending_inbound_entries)]
	pub type PendingInboundEntries<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::SessionId,
		Blake2_128Concat,
		(Proof, T::Hash),
		InboundEntry<T>,
	>;

	/// Storage for inbound routers specific for a domain.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	#[pallet::getter(fn inbound_routers)]
	pub type InboundRouters<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, BoundedVec<T::Hash, T::MaxRouterCount>>;

	/// Storage for the inbound message session IDs.
	#[pallet::storage]
	#[pallet::getter(fn inbound_message_sessions)]
	pub type InboundMessageSessions<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, T::SessionId>;

	/// Storage for inbound message session IDs.
	#[pallet::storage]
	pub type SessionIdStore<T: Config> = StorageValue<_, T::SessionId, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// The origin of the message to be processed is invalid.
		InvalidMessageOrigin,

		/// The domain is not supported.
		DomainNotSupported,

		/// Instance was already added to the domain.
		InstanceAlreadyAdded,

		/// Maximum number of instances for a domain was reached.
		MaxDomainInstances,

		/// Unknown instance.
		UnknownInstance,

		/// Router not found.
		RouterConfigurationNotFound,

		/// Emitted when you call `start_batch_messages()` but that was already
		/// called. You should finalize the message with `end_batch_messages()`
		MessagePackingAlreadyStarted,

		/// Emitted when you can `end_batch_message()` but the packing process
		/// was not started by `start_batch_message()`.
		MessagePackingNotStarted,

		/// Invalid multi router.
		InvalidMultiRouter,

		/// Inbound domain session not found.
		InboundDomainSessionNotFound,

		/// The router that sent the inbound message is unknown.
		UnknownInboundMessageRouter,

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

		/// Multi-router not found.
		MultiRouterNotFound,

		/// Message proof cannot be retrieved.
		MessageProofRetrieval,

		/// Recovery message not found.
		RecoveryMessageNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a known instance of a deployed liquidity pools integration for a
		/// specific domain.
		#[pallet::weight(T::WeightInfo::add_instance())]
		#[pallet::call_index(1)]
		pub fn add_instance(origin: OriginFor<T>, instance: DomainAddress) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			ensure!(
				instance.domain() != Domain::Centrifuge,
				Error::<T>::DomainNotSupported
			);

			ensure!(
				!Allowlist::<T>::contains_key(instance.domain(), instance.clone()),
				Error::<T>::InstanceAlreadyAdded,
			);

			Allowlist::<T>::insert(instance.domain(), instance.clone(), ());

			Self::deposit_event(Event::InstanceAdded { instance });

			Ok(())
		}

		/// Remove an instance from a specific domain.
		#[pallet::weight(T::WeightInfo::remove_instance())]
		#[pallet::call_index(2)]
		pub fn remove_instance(origin: OriginFor<T>, instance: DomainAddress) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			ensure!(
				Allowlist::<T>::contains_key(instance.domain(), instance.clone()),
				Error::<T>::UnknownInstance,
			);

			Allowlist::<T>::remove(instance.domain(), instance.clone());

			Self::deposit_event(Event::InstanceRemoved { instance });

			Ok(())
		}

		/// Process an inbound message.
		#[pallet::weight(T::WeightInfo::receive_message())]
		#[pallet::call_index(5)]
		pub fn receive_message(
			origin: OriginFor<T>,
			router_id: T::RouterId,
			msg: BoundedVec<u8, T::MaxIncomingMessageSize>,
		) -> DispatchResult {
			let GatewayOrigin::Domain(origin_address) = T::LocalEVMOrigin::ensure_origin(origin)?;

			if let DomainAddress::Centrifuge(_) = origin_address {
				return Err(Error::<T>::InvalidMessageOrigin.into());
			}

			Self::receive(router_id, origin_address, msg.into())
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
			DomainHookAddress::<T>::insert(domain.clone(), hook_address);

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
				Some(message) => Self::queue_message(destination, message),
				None => Err(Error::<T>::MessagePackingNotStarted.into()),
			}
		}

		/// Set outbound routers for a particular domain.
		#[pallet::weight(T::WeightInfo::set_outbound_routers())]
		#[pallet::call_index(11)]
		pub fn set_outbound_routers(
			origin: OriginFor<T>,
			domain: Domain,
			routers: BoundedVec<T::Router, T::MaxRouterCount>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			ensure!(domain != Domain::Centrifuge, Error::<T>::DomainNotSupported);

			let mut router_hashes = Vec::new();

			for router in &routers {
				router.init().map_err(|_| Error::<T>::RouterInitFailed)?;

				let router_hash = router.hash();

				router_hashes.push(router_hash);

				OutboundRouters::<T>::insert(router_hash, router);
			}

			<OutboundDomainRouters<T>>::insert(
				domain.clone(),
				BoundedVec::try_from(router_hashes).map_err(|_| Error::<T>::InvalidMultiRouter)?,
			);

			Self::deposit_event(Event::OutboundRoutersSet { domain, routers });

			Ok(())
		}

		/// Set inbound routers.
		#[pallet::weight(T::WeightInfo::set_inbound_routers())]
		#[pallet::call_index(12)]
		pub fn set_inbound_routers(
			origin: OriginFor<T>,
			domain: Domain,
			router_hashes: BoundedVec<T::Hash, T::MaxRouterCount>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			let (old_session_id, new_session_id) = SessionIdStore::<T>::try_mutate(|n| {
				let old_session_id = *n;
				let new_session_id = n.ensure_add(One::one())?;

				*n = new_session_id;

				Ok::<(T::SessionId, T::SessionId), DispatchError>((old_session_id, new_session_id))
			})?;

			InboundRouters::<T>::insert(domain.clone(), router_hashes.clone());
			InboundMessageSessions::<T>::insert(domain.clone(), new_session_id);

			//TODO(cdamian): The storages are updated with the new session.
			// We can process the removal of entries associated with the old entries
			// `on_idle`.
			let _ = PendingInboundEntries::<T>::clear_prefix(old_session_id, u32::MAX, None);

			Self::deposit_event(Event::InboundRoutersSet {
				domain,
				router_hashes,
			});

			Ok(())
		}

		/// Manually increase the proof count for a particular message and
		/// executes it if the required count is reached.
		///
		/// Can only be called by `AdminOrigin`.
		#[pallet::weight(T::WeightInfo::execute_message_recovery())]
		#[pallet::call_index(13)]
		pub fn execute_message_recovery(
			origin: OriginFor<T>,
			message_proof: Proof,
			proof_count: u32,
		) -> DispatchResult {
			//TODO(cdamian): Implement this.
			unimplemented!()
		}
	}

	impl<T: Config> Pallet<T> {
		/// Calculates and returns the proof count required for processing one
		/// inbound message.
		fn get_expected_proof_count(domain: &Domain) -> Result<u32, DispatchError> {
			let routers =
				InboundRouters::<T>::get(domain).ok_or(Error::<T>::MultiRouterNotFound)?;

			let expected_proof_count = routers.len().ensure_sub(1)?;

			Ok(expected_proof_count as u32)
		}

		/// Gets the message proof for a message.
		fn get_message_proof(message: T::Message) -> Proof {
			match message.get_message_proof() {
				None => message
					.to_message_proof()
					.get_message_proof()
					.expect("message proof ensured by 'to_message_proof'"),
				Some(proof) => proof,
			}
		}

		/// Creates an inbound entry based on whether the inbound message is a
		/// proof or not.
		fn create_inbound_entry(
			domain_address: DomainAddress,
			message: T::Message,
			expected_proof_count: u32,
		) -> InboundEntry<T> {
			match message.get_message_proof() {
				None => InboundEntry::Message {
					domain_address,
					message,
					expected_proof_count,
				},
				Some(_) => InboundEntry::Proof { current_count: 1 },
			}
		}

		/// Validation ensures that:
		///
		/// - the router that sent the inbound message is a valid router for the
		///   specific domain.
		/// - messages are only sent by the first inbound router.
		/// - proofs are not sent by the first inbound router.
		fn validate_inbound_entry(
			inbound_processing_info: &InboundProcessingInfo<T>,
			router_hash: T::Hash,
			inbound_entry: &InboundEntry<T>,
		) -> DispatchResult {
			let inbound_routers = inbound_processing_info.inbound_routers.clone();

			ensure!(
				inbound_routers.iter().any(|x| x == &router_hash),
				Error::<T>::UnknownInboundMessageRouter
			);

			match inbound_entry {
				InboundEntry::Message { .. } => {
					ensure!(
						inbound_routers.get(0) == Some(&router_hash),
						Error::<T>::MessageExpectedFromFirstRouter
					);

					Ok(())
				}
				InboundEntry::Proof { .. } => {
					ensure!(
						inbound_routers.get(0) != Some(&router_hash),
						Error::<T>::ProofNotExpectedFromFirstRouter
					);

					Ok(())
				}
			}
		}

		/// Updates the inbound entry for a particular message, increasing the
		/// counts accordingly.
		fn update_pending_entry(
			session_id: T::SessionId,
			message_proof: Proof,
			router_hash: T::Hash,
			inbound_entry: InboundEntry<T>,
			weight: &mut Weight,
		) -> DispatchResult {
			weight.saturating_accrue(T::DbWeight::get().writes(1));

			PendingInboundEntries::<T>::try_mutate(
				session_id,
				(message_proof, router_hash),
				|storage_entry| match storage_entry {
					None => {
						*storage_entry = Some(inbound_entry);

						Ok::<(), DispatchError>(())
					}
					Some(stored_inbound_entry) => match stored_inbound_entry {
						InboundEntry::Message {
							expected_proof_count: old,
							..
						} => match inbound_entry {
							InboundEntry::Message {
								expected_proof_count: new,
								..
							} => old.ensure_add_assign(new).map_err(|e| e.into()),
							InboundEntry::Proof { .. } => {
								Err(Error::<T>::ExpectedMessageType.into())
							}
						},
						InboundEntry::Proof { current_count: old } => match inbound_entry {
							InboundEntry::Proof { current_count: new } => {
								old.ensure_add_assign(new).map_err(|e| e.into())
							}
							InboundEntry::Message { .. } => {
								Err(Error::<T>::ExpectedMessageProofType.into())
							}
						},
					},
				},
			)
		}

		/// Creates, validates and updates the inbound entry.
		fn validate_and_update_pending_entries(
			inbound_processing_info: &InboundProcessingInfo<T>,
			message: T::Message,
			message_proof: Proof,
			router_hash: T::Hash,
			weight: &mut Weight,
		) -> DispatchResult {
			let inbound_entry = Self::create_inbound_entry(
				inbound_processing_info.domain_address.clone(),
				message,
				inbound_processing_info.expected_proof_count_per_message,
			);

			Self::validate_inbound_entry(&inbound_processing_info, router_hash, &inbound_entry)?;

			Self::update_pending_entry(
				inbound_processing_info.current_session_id,
				message_proof,
				router_hash,
				inbound_entry,
				weight,
			)?;

			Ok(())
		}

		/// Checks if the number of proofs required for executing one message
		/// were received, and returns the message if so.
		fn get_executable_message(
			inbound_processing_info: &InboundProcessingInfo<T>,
			message_proof: Proof,
		) -> Option<T::Message> {
			let mut message = None;
			let mut votes = 0;

			for inbound_router in &inbound_processing_info.inbound_routers {
				match PendingInboundEntries::<T>::get(
					inbound_processing_info.current_session_id,
					(message_proof, inbound_router),
				) {
					// We expected one InboundEntry for each router, if that's not the case,
					// we can return.
					None => return None,
					Some(inbound_entry) => match inbound_entry {
						InboundEntry::Message {
							message: stored_message,
							..
						} => message = Some(stored_message),
						InboundEntry::Proof { current_count } => {
							if current_count > 0 {
								votes += 1;
							}
						}
					},
				};
			}

			if votes == inbound_processing_info.expected_proof_count_per_message {
				return message;
			}

			None
		}

		/// Decreases the counts for inbound entries and removes them if the
		/// counts reach 0.
		fn decrease_pending_entries_counts(
			inbound_processing_info: &InboundProcessingInfo<T>,
			message_proof: Proof,
		) -> DispatchResult {
			for inbound_router in &inbound_processing_info.inbound_routers {
				match PendingInboundEntries::<T>::try_mutate(
					inbound_processing_info.current_session_id,
					(message_proof, inbound_router),
					|storage_entry| match storage_entry {
						None => Err(Error::<T>::PendingInboundEntryNotFound.into()),
						Some(stored_inbound_entry) => match stored_inbound_entry {
							InboundEntry::Message {
								expected_proof_count,
								..
							} => {
								let updated_count = (*expected_proof_count).ensure_sub(
									inbound_processing_info.expected_proof_count_per_message,
								)?;

								if updated_count == 0 {
									*storage_entry = None;
								} else {
									*expected_proof_count = updated_count;
								}

								Ok::<(), DispatchError>(())
							}
							InboundEntry::Proof { current_count } => {
								let updated_count = (*current_count).ensure_sub(1)?;

								if updated_count == 0 {
									*storage_entry = None;
								} else {
									*current_count = updated_count;
								}

								Ok::<(), DispatchError>(())
							}
						},
					},
				) {
					Ok(()) => {}
					Err(e) => return Err(e),
				}
			}

			Ok(())
		}

		/// Retrieves the information required for processing an inbound
		/// message.
		fn get_inbound_processing_info(
			domain_address: DomainAddress,
			weight: &mut Weight,
		) -> Result<InboundProcessingInfo<T>, DispatchError> {
			let inbound_routers = InboundRouters::<T>::get(domain_address.domain())
				.ok_or(Error::<T>::MultiRouterNotFound)?;

			weight.saturating_accrue(T::DbWeight::get().reads(1));

			let current_session_id = InboundMessageSessions::<T>::get(domain_address.domain())
				.ok_or(Error::<T>::InboundDomainSessionNotFound)?;

			weight.saturating_accrue(T::DbWeight::get().reads(1));

			let expected_proof_count = Self::get_expected_proof_count(&domain_address.domain())?;

			weight.saturating_accrue(T::DbWeight::get().reads(1));

			Ok(InboundProcessingInfo {
				domain_address,
				inbound_routers,
				current_session_id,
				expected_proof_count_per_message: expected_proof_count,
			})
		}

		/// Iterates over a batch of messages and checks if the requirements for
		/// processing each message are met.
		fn process_inbound_message(
			domain_address: DomainAddress,
			message: T::Message,
			router_hash: T::Hash,
		) -> (DispatchResult, Weight) {
			let mut weight = Default::default();

			let inbound_processing_info =
				match Self::get_inbound_processing_info(domain_address.clone(), &mut weight) {
					Ok(i) => i,
					Err(e) => return (Err(e), weight),
				};

			weight.saturating_accrue(
				Weight::from_parts(0, T::Message::max_encoded_len() as u64)
					.saturating_add(LP_DEFENSIVE_WEIGHT),
			);

			let mut count = 0;

			for submessage in message.submessages() {
				count += 1;

				let message_proof = Self::get_message_proof(message.clone());

				if let Err(e) = Self::validate_and_update_pending_entries(
					&inbound_processing_info,
					submessage.clone(),
					message_proof,
					router_hash,
					&mut weight,
				) {
					return (Err(e), weight);
				}

				match Self::get_executable_message(&inbound_processing_info, message_proof) {
					Some(m) => {
						if let Err(e) = Self::decrease_pending_entries_counts(
							&inbound_processing_info,
							message_proof,
						) {
							return (Err(e), weight.saturating_mul(count));
						}

						if let Err(e) = T::InboundMessageHandler::handle(domain_address.clone(), m)
						{
							// We only consume the processed weight if error during the batch
							return (Err(e), weight.saturating_mul(count));
						}
					}
					None => continue,
				}
			}

			(Ok(()), LP_DEFENSIVE_WEIGHT.saturating_mul(count))
		}

		/// Retrieves the stored router, sends the message, and calculates and
		/// returns the router operation result and the weight used.
		fn process_outbound_message(
			sender: T::AccountId,
			message: T::Message,
			router_hash: T::Hash,
		) -> (DispatchResult, Weight) {
			let router_ids = T::RouterId::for_domain(domain);

			let Some(router) = OutboundRouters::<T>::get(router_hash) else {
				return (Err(Error::<T>::RouterNotFound.into()), read_weight);
			};

			let mut count = 0;
			let bytes = message.serialize();

			for router_id in router_ids {
				count += 1;
				if let Err(e) = T::MessageSender::send(router_id, sender.clone(), bytes.clone()) {
					return (Err(e), LP_DEFENSIVE_WEIGHT.saturating_mul(count));
				}
			}

			// TODO: Should we fix weights?
			(Ok(()), LP_DEFENSIVE_WEIGHT.saturating_mul(count))
		}

		/// Retrieves the hashes of the routers set for a domain and queues the
		/// message and proofs accordingly.
		fn queue_message(destination: Domain, message: T::Message) -> DispatchResult {
			let router_hashes = OutboundDomainRouters::<T>::get(destination.clone())
				.ok_or(Error::<T>::MultiRouterNotFound)?;

			let message_proof = message.to_message_proof();
			let mut message_opt = Some(message);

			for router_hash in router_hashes {
				// Ensure that we only send the actual message once, using one router.
				// The remaining routers will send the message proof.
				let router_msg = match message_opt.take() {
					Some(m) => m,
					None => message_proof.clone(),
				};

				// We are using the sender specified in the pallet config so that we can
				// ensure that the account is funded
				let gateway_message =
					GatewayMessage::<T::AccountId, T::Message, T::Hash>::Outbound {
						sender: T::Sender::get(),
						message: router_msg,
						router_hash,
					};

				T::MessageQueue::submit(gateway_message)?;
			}

			Ok(())
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

			PackedMessage::<T>::mutate((&from, destination.clone()), |batch| match batch {
				Some(batch) => batch.pack_with(message),
				None => Self::queue_message(destination, message),
			})
		}
	}

	impl<T: Config> GetByKey<Domain, Option<[u8; 20]>> for Pallet<T> {
		fn get(domain: &Domain) -> Option<[u8; 20]> {
			DomainHookAddress::<T>::get(domain)
		}
	}

	impl<T: Config> MessageProcessor for Pallet<T> {
		type Message = GatewayMessage<T::AccountId, T::Message, T::Hash>;

		fn process(msg: Self::Message) -> (DispatchResult, Weight) {
			match msg {
				GatewayMessage::Inbound {
					domain_address,
					message,
					router_hash,
				} => Self::process_inbound_message(domain_address, message, router_hash),
				GatewayMessage::Outbound {
					sender,
					message,
					router_hash,
				} => Self::process_outbound_message(sender, message, router_hash),
			}
		}

		/// Returns the max processing weight for a message, based on its
		/// direction.
		fn max_processing_weight(msg: &Self::Message) -> Weight {
			match msg {
				GatewayMessage::Inbound { message, .. } => {
					LP_DEFENSIVE_WEIGHT.saturating_mul(message.submessages().len() as u64)
				}
				GatewayMessage::Outbound { .. } => LP_DEFENSIVE_WEIGHT,
			}
		}
	}

	impl<T: Config> MessageReceiver for Pallet<T> {
		type Middleware = T::RouterId;
		type Origin = DomainAddress;

		fn receive(
			_router_id: T::RouterId,
			origin_address: DomainAddress,
			message: Vec<u8>,
		) -> DispatchResult {
			// TODO handle router ids logic with votes and session_id

			ensure!(
				Allowlist::<T>::contains_key(origin_address.domain(), origin_address.clone()),
				Error::<T>::UnknownInstance,
			);

			let gateway_message = GatewayMessage::<T::Message>::Inbound {
				domain_address: origin_address,
				message: T::Message::deserialize(&message)?,
			};

			T::MessageQueue::submit(gateway_message)
		}
	}
}
