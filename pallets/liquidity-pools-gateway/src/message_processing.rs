use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{InboundMessageHandler, LPEncoding, MessageQueue, Proof, Router};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::{Decode, Encode, Get, TypeInfo},
	weights::Weight,
	BoundedVec,
};
use parity_scale_codec::MaxEncodedLen;
use sp_arithmetic::traits::{EnsureAddAssign, EnsureSub};
use sp_runtime::DispatchError;

use crate::{
	message::GatewayMessage, Config, Error, InboundMessageSessions, InboundRouters,
	OutboundDomainRouters, OutboundRouters, Pallet, PendingInboundEntries,
};

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

impl<T: Config> Pallet<T> {
	/// Calculates and returns the proof count required for processing one
	/// inbound message.
	fn get_expected_proof_count(domain: &Domain) -> Result<u32, DispatchError> {
		let routers = InboundRouters::<T>::get(domain).ok_or(Error::<T>::MultiRouterNotFound)?;

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
						InboundEntry::Proof { .. } => Err(Error::<T>::ExpectedMessageType.into()),
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
	pub(crate) fn process_inbound_message(
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

					if let Err(e) = T::InboundMessageHandler::handle(domain_address.clone(), m) {
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
	pub(crate) fn process_outbound_message(
		sender: T::AccountId,
		message: T::Message,
		router_hash: T::Hash,
	) -> (DispatchResult, Weight) {
		let read_weight = T::DbWeight::get().reads(1);

		let Some(router) = OutboundRouters::<T>::get(router_hash) else {
			return (Err(Error::<T>::RouterNotFound.into()), read_weight);
		};

		let (result, router_weight) = match router.send(sender, message.serialize()) {
			Ok(dispatch_info) => (Ok(()), dispatch_info.actual_weight),
			Err(e) => (Err(e.error), e.post_info.actual_weight),
		};

		(result, router_weight.unwrap_or(read_weight))
	}

	/// Retrieves the hashes of the routers set for a domain and queues the
	/// message and proofs accordingly.
	pub(crate) fn queue_message(destination: Domain, message: T::Message) -> DispatchResult {
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
			let gateway_message = GatewayMessage::<T::AccountId, T::Message, T::Hash>::Outbound {
				sender: T::Sender::get(),
				message: router_msg,
				router_hash,
			};

			T::MessageQueue::submit(gateway_message)?;
		}

		Ok(())
	}
}
