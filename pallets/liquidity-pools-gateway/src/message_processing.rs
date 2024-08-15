use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{
	InboundMessageHandler, LPEncoding, MessageQueue, MessageSender, Proof, RouterProvider,
};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::{Decode, Encode, Get, TypeInfo},
	weights::Weight,
};
use parity_scale_codec::MaxEncodedLen;
use sp_arithmetic::traits::{EnsureAddAssign, EnsureSub, SaturatedConversion};
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

use crate::{
	message::GatewayMessage, Config, Error, Pallet, PendingInboundEntries, Routers, SessionIdStore,
};

/// Type that holds the information needed for inbound message entries.
#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct MessageEntry<T: Config> {
	/// The session ID for this entry.
	pub session_id: T::SessionId,

	/// The sender of the inbound message.
	pub domain_address: DomainAddress,

	/// The LP message.
	pub message: T::Message,

	/// The expected proof count for processing one or more of the provided
	/// message.
	///
	/// NOTE - this gets increased by the `expected_proof_count` for a set of
	/// routers (see `get_expected_proof_count`) every time a new identical
	/// message is submitted.
	pub expected_proof_count: u32,
}

/// Type that holds the information needed for inbound proof entries.
#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ProofEntry<T: Config> {
	/// The session ID for this entry.
	pub session_id: T::SessionId,

	/// The number of proofs received for a particular message.
	///
	/// NOTE - this gets increased by 1 every time a new identical message is
	/// submitted.
	pub current_count: u32,
}

impl<T: Config> ProofEntry<T> {
	/// Returns `true` if all the following conditions are true:
	/// - the session IDs match
	/// - the `current_count` is greater than 0
	pub fn has_valid_vote_for_session(&self, session_id: T::SessionId) -> bool {
		self.session_id == session_id && self.current_count > 0
	}
}

/// Type used when storing inbound message information.
#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub enum InboundEntry<T: Config> {
	Message(MessageEntry<T>),
	Proof(ProofEntry<T>),
}

impl<T: Config> From<(&InboundProcessingInfo<T>, T::Message)> for InboundEntry<T> {
	fn from((inbound_processing_info, message): (&InboundProcessingInfo<T>, T::Message)) -> Self {
		match message.proof_hash() {
			None => InboundEntry::Message(MessageEntry {
				session_id: inbound_processing_info.current_session_id,
				domain_address: inbound_processing_info.domain_address.clone(),
				message,
				expected_proof_count: inbound_processing_info.expected_proof_count_per_message,
			}),
			Some(_) => InboundEntry::Proof(ProofEntry {
				session_id: inbound_processing_info.current_session_id,
				current_count: 1,
			}),
		}
	}
}

impl<T: Config> From<MessageEntry<T>> for InboundEntry<T> {
	fn from(message_entry: MessageEntry<T>) -> Self {
		Self::Message(message_entry)
	}
}

impl<T: Config> From<ProofEntry<T>> for InboundEntry<T> {
	fn from(proof_entry: ProofEntry<T>) -> Self {
		Self::Proof(proof_entry)
	}
}

impl<T: Config> InboundEntry<T> {
	/// Creates a new `InboundEntry` based on the information provided.
	///
	/// If a session change is detected or if the updated counts reach 0, it
	/// means that a new entry is no longer required, otherwise, the counts are
	/// decreased accordingly, based on the entry type.
	pub fn create_post_voting_entry(
		inbound_entry: &InboundEntry<T>,
		inbound_processing_info: &InboundProcessingInfo<T>,
	) -> Result<Option<Self>, DispatchError> {
		match inbound_entry {
			InboundEntry::Message(message_entry) => {
				if message_entry.session_id != inbound_processing_info.current_session_id {
					return Ok(None);
				}

				let updated_count = message_entry
					.expected_proof_count
					.ensure_sub(inbound_processing_info.expected_proof_count_per_message)?;

				if updated_count == 0 {
					return Ok(None);
				}

				Ok(Some(
					MessageEntry {
						expected_proof_count: updated_count,
						..message_entry.clone()
					}
					.into(),
				))
			}
			InboundEntry::Proof(proof_entry) => {
				if proof_entry.session_id != inbound_processing_info.current_session_id {
					return Ok(None);
				}

				let updated_count = proof_entry.current_count.ensure_sub(1)?;

				if updated_count == 0 {
					return Ok(None);
				}

				Ok(Some(
					ProofEntry {
						current_count: updated_count,
						..proof_entry.clone()
					}
					.into(),
				))
			}
		}
	}

	/// Validation ensures that:
	///
	/// - the router that sent the inbound message is a valid router for the
	///   specific domain.
	/// - messages are only sent by the first inbound router.
	/// - proofs are not sent by the first inbound router.
	pub fn validate(
		&self,
		inbound_processing_info: &InboundProcessingInfo<T>,
		router_id: &T::RouterId,
	) -> DispatchResult {
		let router_ids = inbound_processing_info.router_ids.clone();

		ensure!(
			router_ids.iter().any(|x| x == router_id),
			Error::<T>::UnknownRouter
		);

		match self {
			InboundEntry::Message { .. } => {
				ensure!(
					router_ids.first() == Some(router_id),
					Error::<T>::MessageExpectedFromFirstRouter
				);

				Ok(())
			}
			InboundEntry::Proof { .. } => {
				ensure!(
					router_ids.first() != Some(router_id),
					Error::<T>::ProofNotExpectedFromFirstRouter
				);

				Ok(())
			}
		}
	}

	/// Checks if the entry type is a proof and increments the count by 1
	/// or sets it to 1 if the session is changed.
	pub fn increment_proof_count(&mut self, session_id: T::SessionId) -> DispatchResult {
		match self {
			InboundEntry::Proof(proof_entry) => {
				if proof_entry.session_id != session_id {
					proof_entry.session_id = session_id;
					proof_entry.current_count = 1;
				} else {
					proof_entry.current_count.ensure_add_assign(1)?;
				}

				Ok::<(), DispatchError>(())
			}
			InboundEntry::Message(_) => Err(Error::<T>::ExpectedMessageProofType.into()),
		}
	}

	/// A pre-dispatch update involves increasing the `expected_proof_count` or
	/// `current_count` of `self` with the one of `other`.
	///
	/// If a session ID change is detected, `self` is replaced completely by
	/// `other`.
	pub fn pre_dispatch_update(&mut self, other: Self) -> DispatchResult {
		match (&mut *self, &other) {
			// Message entries
			(
				InboundEntry::Message(self_message_entry),
				InboundEntry::Message(other_message_entry),
			) => {
				if self_message_entry.session_id != other_message_entry.session_id {
					*self = other;

					return Ok(());
				}

				self_message_entry
					.expected_proof_count
					.ensure_add_assign(other_message_entry.expected_proof_count)?;

				Ok(())
			}
			// Proof entries
			(InboundEntry::Proof(self_proof_entry), InboundEntry::Proof(other_proof_entry)) => {
				if self_proof_entry.session_id != other_proof_entry.session_id {
					*self = other;

					return Ok(());
				}

				self_proof_entry
					.current_count
					.ensure_add_assign(other_proof_entry.current_count)?;

				Ok(())
			}
			// Mismatches
			(InboundEntry::Message(_), InboundEntry::Proof(_)) => {
				Err(Error::<T>::ExpectedMessageType.into())
			}
			(InboundEntry::Proof(_), InboundEntry::Message(_)) => {
				Err(Error::<T>::ExpectedMessageProofType.into())
			}
		}
	}
}

/// Type that holds information used when processing inbound messages.
#[derive(Clone)]
pub struct InboundProcessingInfo<T: Config> {
	pub domain_address: DomainAddress,
	pub router_ids: Vec<T::RouterId>,
	pub current_session_id: T::SessionId,
	pub expected_proof_count_per_message: u32,
}

impl<T: Config> TryFrom<DomainAddress> for InboundProcessingInfo<T> {
	type Error = DispatchError;

	fn try_from(domain_address: DomainAddress) -> Result<Self, Self::Error> {
		let router_ids = Pallet::<T>::get_router_ids_for_domain(domain_address.domain())?;

		let current_session_id = SessionIdStore::<T>::get();

		let expected_proof_count = Pallet::<T>::get_expected_proof_count(&router_ids)?;

		Ok(InboundProcessingInfo {
			domain_address,
			router_ids,
			current_session_id,
			expected_proof_count_per_message: expected_proof_count,
		})
	}
}

impl<T: Config> Pallet<T> {
	/// Retrieves all stored routers and then filters them based
	/// on the available routers for the provided domain.
	pub(crate) fn get_router_ids_for_domain(
		domain: Domain,
	) -> Result<Vec<T::RouterId>, DispatchError> {
		let stored_routers = Routers::<T>::get();

		let all_routers_for_domain = T::RouterProvider::routers_for_domain(domain);

		let res = stored_routers
			.iter()
			.filter(|stored_router| {
				all_routers_for_domain
					.iter()
					.any(|available_router| *stored_router == available_router)
			})
			.cloned()
			.collect::<Vec<_>>();

		if res.is_empty() {
			return Err(Error::<T>::NotEnoughRoutersForDomain.into());
		}

		Ok(res)
	}

	/// Calculates and returns the proof count required for processing one
	/// inbound message.
	pub(crate) fn get_expected_proof_count(
		router_ids: &[T::RouterId],
	) -> Result<u32, DispatchError> {
		let expected_proof_count = router_ids
			.len()
			.ensure_sub(1)
			.map_err(|_| Error::<T>::NotEnoughRoutersForDomain)?;

		Ok(expected_proof_count.saturated_into())
	}

	/// Gets the message proof for a message.
	pub(crate) fn get_message_proof(message: T::Message) -> Proof {
		match message.proof_hash() {
			None => message
				.proof_message()
				.proof_hash()
				.expect("message proof ensured by 'to_message_proof'"),
			Some(proof) => proof,
		}
	}

	/// Upserts an inbound entry for a particular message, increasing the
	/// relevant counts accordingly.
	pub(crate) fn upsert_pending_entry(
		message_proof: Proof,
		router_id: T::RouterId,
		new_inbound_entry: InboundEntry<T>,
	) -> DispatchResult {
		PendingInboundEntries::<T>::try_mutate(message_proof, router_id, |storage_entry| {
			match storage_entry {
				None => {
					*storage_entry = Some(new_inbound_entry);

					Ok::<(), DispatchError>(())
				}
				Some(stored_inbound_entry) => {
					stored_inbound_entry.pre_dispatch_update(new_inbound_entry)
				}
			}
		})
	}

	/// Creates, validates and upserts the inbound entry.
	fn validate_and_upsert_pending_entries(
		inbound_processing_info: &InboundProcessingInfo<T>,
		message: T::Message,
		message_proof: Proof,
		router_id: T::RouterId,
	) -> DispatchResult {
		let inbound_entry: InboundEntry<T> = (inbound_processing_info, message).into();

		inbound_entry.validate(inbound_processing_info, &router_id)?;

		Self::upsert_pending_entry(message_proof, router_id, inbound_entry)?;

		Ok(())
	}

	/// Checks if the number of proofs required for executing one message
	/// were received, and if so, decreases the counts accordingly and executes
	/// the message.
	pub(crate) fn execute_if_requirements_are_met(
		inbound_processing_info: &InboundProcessingInfo<T>,
		message_proof: Proof,
	) -> DispatchResult {
		let mut message = None;
		let mut votes = 0;

		for router_id in &inbound_processing_info.router_ids {
			match PendingInboundEntries::<T>::get(message_proof, router_id) {
				// We expected one InboundEntry for each router, if that's not the case,
				// we can return.
				None => return Ok(()),
				Some(stored_inbound_entry) => match stored_inbound_entry {
					InboundEntry::Message(message_entry) => message = Some(message_entry.message),
					InboundEntry::Proof(proof_entry) => {
						if proof_entry
							.has_valid_vote_for_session(inbound_processing_info.current_session_id)
						{
							votes.ensure_add_assign(1)?;
						}
					}
				},
			};
		}

		if votes < inbound_processing_info.expected_proof_count_per_message {
			return Ok(());
		}

		match message {
			Some(msg) => {
				Self::execute_post_voting_dispatch(inbound_processing_info, message_proof)?;

				T::InboundMessageHandler::handle(
					inbound_processing_info.domain_address.clone(),
					msg,
				)
			}
			None => Ok(()),
		}
	}

	/// Decreases the counts for inbound entries and removes them if the
	/// counts reach 0.
	pub(crate) fn execute_post_voting_dispatch(
		inbound_processing_info: &InboundProcessingInfo<T>,
		message_proof: Proof,
	) -> DispatchResult {
		for router_id in &inbound_processing_info.router_ids {
			PendingInboundEntries::<T>::try_mutate(message_proof, router_id, |storage_entry| {
				match storage_entry {
					None => {
						Err::<(), DispatchError>(Error::<T>::PendingInboundEntryNotFound.into())
					}
					Some(stored_inbound_entry) => {
						let post_dispatch_entry = InboundEntry::create_post_voting_entry(
							stored_inbound_entry,
							inbound_processing_info,
						)?;

						*storage_entry = post_dispatch_entry;

						Ok(())
					}
				}
			})?;
		}

		Ok(())
	}

	/// Iterates over a batch of messages and checks if the requirements for
	/// processing each message are met.
	pub(crate) fn process_inbound_message(
		domain_address: DomainAddress,
		message: T::Message,
		router_id: T::RouterId,
	) -> (DispatchResult, Weight) {
		let weight = LP_DEFENSIVE_WEIGHT;

		let inbound_processing_info = match domain_address.clone().try_into() {
			Ok(i) => i,
			Err(e) => return (Err(e), weight),
		};

		let mut count = 0;

		for submessage in message.submessages() {
			if let Err(e) = count.ensure_add_assign(1) {
				return (Err(e.into()), weight.saturating_mul(count));
			}

			let message_proof = Self::get_message_proof(message.clone());

			if let Err(e) = Self::validate_and_upsert_pending_entries(
				&inbound_processing_info,
				submessage.clone(),
				message_proof,
				router_id.clone(),
			) {
				return (Err(e), weight.saturating_mul(count));
			}

			match Self::execute_if_requirements_are_met(&inbound_processing_info, message_proof) {
				Err(e) => return (Err(e), weight.saturating_mul(count)),
				Ok(_) => continue,
			}
		}

		(Ok(()), weight.saturating_mul(count))
	}

	/// Retrieves the stored router, sends the message, and calculates and
	/// returns the router operation result and the weight used.
	pub(crate) fn process_outbound_message(
		sender: DomainAddress,
		message: T::Message,
		router_id: T::RouterId,
	) -> (DispatchResult, Weight) {
		let weight = LP_DEFENSIVE_WEIGHT;

		match T::MessageSender::send(router_id, sender, message.serialize()) {
			Ok(_) => (Ok(()), weight),
			Err(e) => (Err(e), weight),
		}
	}

	/// Retrieves the IDs of the routers set for a domain and queues the
	/// message and proofs accordingly.
	pub(crate) fn queue_outbound_message(
		destination: Domain,
		message: T::Message,
	) -> DispatchResult {
		let router_ids = Self::get_router_ids_for_domain(destination)?;

		let message_proof = message.proof_message();
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
			let gateway_message = GatewayMessage::<T::Message, T::RouterId>::Outbound {
				sender: T::Sender::get(),
				message: router_msg,
				router_id,
			};

			T::MessageQueue::submit(gateway_message)?;
		}

		Ok(())
	}
}
