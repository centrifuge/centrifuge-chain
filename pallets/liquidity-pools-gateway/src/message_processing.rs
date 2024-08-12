use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{InboundMessageHandler, LPEncoding, MessageQueue, Proof};
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
	message::GatewayMessage, Config, Error, InboundMessageSessions, InvalidMessageSessions, Pallet,
	PendingInboundEntries, Routers,
};

/// The limit used when clearing the `PendingInboundEntries` for invalid
/// session IDs.
const INVALID_ID_REMOVAL_LIMIT: u32 = 100;

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
	routers: BoundedVec<T::RouterId, T::MaxRouterCount>,
	current_session_id: T::SessionId,
	expected_proof_count_per_message: u32,
}

impl<T: Config> Pallet<T> {
	/// Calculates and returns the proof count required for processing one
	/// inbound message.
	fn get_expected_proof_count(domain: &Domain) -> Result<u32, DispatchError> {
		let routers = Routers::<T>::get(domain).ok_or(Error::<T>::RoutersNotFound)?;

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
		router_id: &T::RouterId,
		inbound_entry: &InboundEntry<T>,
	) -> DispatchResult {
		let routers = inbound_processing_info.routers.clone();

		ensure!(
			routers.iter().any(|x| x == router_id),
			Error::<T>::UnknownInboundMessageRouter
		);

		match inbound_entry {
			InboundEntry::Message { .. } => {
				ensure!(
					routers.get(0) == Some(&router_id),
					Error::<T>::MessageExpectedFromFirstRouter
				);

				Ok(())
			}
			InboundEntry::Proof { .. } => {
				ensure!(
					routers.get(0) != Some(&router_id),
					Error::<T>::ProofNotExpectedFromFirstRouter
				);

				Ok(())
			}
		}
	}

	/// Upserts an inbound entry for a particular message, increasing the
	/// relevant counts accordingly.
	fn upsert_pending_entry(
		session_id: T::SessionId,
		message_proof: Proof,
		router_id: T::RouterId,
		inbound_entry: InboundEntry<T>,
		weight: &mut Weight,
	) -> DispatchResult {
		weight.saturating_accrue(T::DbWeight::get().writes(1));

		PendingInboundEntries::<T>::try_mutate(
			session_id,
			(message_proof, router_id),
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

	/// Creates, validates and upserts the inbound entry.
	fn validate_and_upsert_pending_entries(
		inbound_processing_info: &InboundProcessingInfo<T>,
		message: T::Message,
		message_proof: Proof,
		router_id: T::RouterId,
		weight: &mut Weight,
	) -> DispatchResult {
		let inbound_entry = Self::create_inbound_entry(
			inbound_processing_info.domain_address.clone(),
			message,
			inbound_processing_info.expected_proof_count_per_message,
		);

		Self::validate_inbound_entry(&inbound_processing_info, &router_id, &inbound_entry)?;

		Self::upsert_pending_entry(
			inbound_processing_info.current_session_id,
			message_proof,
			router_id,
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
		weight: &mut Weight,
	) -> Option<T::Message> {
		let mut message = None;
		let mut votes = 0;

		for router in &inbound_processing_info.routers {
			weight.saturating_accrue(T::DbWeight::get().reads(1));

			match PendingInboundEntries::<T>::get(
				inbound_processing_info.current_session_id,
				(message_proof, router),
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
		weight: &mut Weight,
	) -> DispatchResult {
		for router in &inbound_processing_info.routers {
			weight.saturating_accrue(T::DbWeight::get().writes(1));

			match PendingInboundEntries::<T>::try_mutate(
				inbound_processing_info.current_session_id,
				(message_proof, router),
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
		let routers =
			Routers::<T>::get(domain_address.domain()).ok_or(Error::<T>::RoutersNotFound)?;

		weight.saturating_accrue(T::DbWeight::get().reads(1));

		let current_session_id = InboundMessageSessions::<T>::get(domain_address.domain())
			.ok_or(Error::<T>::InboundDomainSessionNotFound)?;

		weight.saturating_accrue(T::DbWeight::get().reads(1));

		let expected_proof_count = Self::get_expected_proof_count(&domain_address.domain())?;

		weight.saturating_accrue(T::DbWeight::get().reads(1));

		Ok(InboundProcessingInfo {
			domain_address,
			routers,
			current_session_id,
			expected_proof_count_per_message: expected_proof_count,
		})
	}

	/// Iterates over a batch of messages and checks if the requirements for
	/// processing each message are met.
	pub(crate) fn process_inbound_message(
		domain_address: DomainAddress,
		message: T::Message,
		router_id: T::RouterId,
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

			if let Err(e) = Self::validate_and_upsert_pending_entries(
				&inbound_processing_info,
				submessage.clone(),
				message_proof,
				router_id.clone(),
				&mut weight,
			) {
				return (Err(e), weight);
			}

			match Self::get_executable_message(&inbound_processing_info, message_proof, &mut weight)
			{
				Some(m) => {
					if let Err(e) = Self::decrease_pending_entries_counts(
						&inbound_processing_info,
						message_proof,
						&mut weight,
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

		(Ok(()), weight.saturating_mul(count))
	}

	/// Retrieves the stored router, sends the message, and calculates and
	/// returns the router operation result and the weight used.
	pub(crate) fn process_outbound_message(
		sender: T::AccountId,
		message: T::Message,
		router_id: T::RouterId,
	) -> (DispatchResult, Weight) {
		let read_weight = T::DbWeight::get().reads(1);

		// TODO(cdamian): Update when the router refactor is done.

		// let Some(router) = Routers::<T>::get(router_id) else {
		// 	return (Err(Error::<T>::RouterNotFound.into()), read_weight);
		// };
		//
		// let (result, router_weight) = match router.send(sender, message.serialize())
		// { 	Ok(dispatch_info) => (Ok(()), dispatch_info.actual_weight),
		// 	Err(e) => (Err(e.error), e.post_info.actual_weight),
		// };
		//
		// (result, router_weight.unwrap_or(read_weight))

		(Ok(()), read_weight)
	}

	/// Retrieves the IDs of the routers set for a domain and queues the
	/// message and proofs accordingly.
	pub(crate) fn queue_message(destination: Domain, message: T::Message) -> DispatchResult {
		let router_ids =
			Routers::<T>::get(destination.clone()).ok_or(Error::<T>::RoutersNotFound)?;

		let message_proof = message.to_message_proof();
		let mut message_opt = Some(message);

		for router_id in router_ids {
			// Ensure that we only send the actual message once, using one router.
			// The remaining routers will send the message proof.
			let router_msg = match message_opt.take() {
				Some(m) => m,
				None => message_proof.clone(),
			};

			// We are using the sender specified in the pallet config so that we can
			// ensure that the account is funded
			let gateway_message =
				GatewayMessage::<T::AccountId, T::Message, T::RouterId>::Outbound {
					sender: T::Sender::get(),
					message: router_msg,
					router_id,
				};

			T::MessageQueue::submit(gateway_message)?;
		}

		Ok(())
	}

	/// Clears `PendingInboundEntries` mapped to invalid session IDs as long as
	/// there is enough weight available for this operation.
	///
	/// The invalid session IDs are removed from storage if all entries mapped
	/// to them were cleared.
	pub(crate) fn clear_invalid_session_ids(max_weight: Weight) -> Weight {
		let invalid_session_ids = InvalidMessageSessions::<T>::iter_keys().collect::<Vec<_>>();

		let mut weight = T::DbWeight::get().reads(1);

		for invalid_session_id in invalid_session_ids {
			let mut cursor: Option<Vec<u8>> = None;

			loop {
				let res = PendingInboundEntries::<T>::clear_prefix(
					invalid_session_id,
					INVALID_ID_REMOVAL_LIMIT,
					cursor.as_ref().map(|x| x.as_ref()),
				);

				weight.saturating_accrue(
					T::DbWeight::get().reads_writes(res.loops.into(), res.unique.into()),
				);

				if weight.all_gte(max_weight) {
					return weight;
				}

				cursor = match res.maybe_cursor {
					None => {
						InvalidMessageSessions::<T>::remove(invalid_session_id);

						weight.saturating_accrue(T::DbWeight::get().writes(1));

						if weight.all_gte(max_weight) {
							return weight;
						}

						break;
					}
					Some(c) => Some(c),
				};
			}
		}

		weight
	}
}
