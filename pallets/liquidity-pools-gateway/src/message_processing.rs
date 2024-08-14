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
use sp_arithmetic::traits::{EnsureAddAssign, EnsureSub};
use sp_runtime::DispatchError;

use crate::{
	message::GatewayMessage, Config, Error, Pallet, PendingInboundEntries, Routers, SessionIdStore,
};

/// Type used when storing inbound message information.
#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub enum InboundEntry<T: Config> {
	Message {
		session_id: T::SessionId,
		domain_address: DomainAddress,
		message: T::Message,
		expected_proof_count: u32,
	},
	Proof {
		session_id: T::SessionId,
		current_count: u32,
	},
}

/// Type used when processing inbound messages.
#[derive(Clone)]
pub struct InboundProcessingInfo<T: Config> {
	pub domain_address: DomainAddress,
	pub router_ids: Vec<T::RouterId>,
	pub current_session_id: T::SessionId,
	pub expected_proof_count_per_message: u32,
}

impl<T: Config> Pallet<T> {
	/// Retrieves all available routers for a domain and then filters them based
	/// on the routers that we have in storage.
	pub fn get_router_ids_for_domain(domain: Domain) -> Result<Vec<T::RouterId>, DispatchError> {
		let all_routers_for_domain = T::RouterProvider::routers_for_domain(domain);

		let stored_routers = Routers::<T>::get();

		let res = all_routers_for_domain
			.iter()
			.filter(|x| stored_routers.iter().any(|y| *x == y))
			.map(|x| x.clone())
			.collect::<Vec<_>>();

		if res.is_empty() {
			return Err(Error::<T>::NotEnoughRoutersForDomain.into());
		}

		Ok(res)
	}

	/// Calculates and returns the proof count required for processing one
	/// inbound message.
	fn get_expected_proof_count(router_ids: &Vec<T::RouterId>) -> Result<u32, DispatchError> {
		let expected_proof_count = router_ids
			.len()
			.ensure_sub(1)
			.map_err(|_| Error::<T>::NotEnoughRoutersForDomain)?;

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
		inbound_processing_info: &InboundProcessingInfo<T>,
		message: T::Message,
	) -> InboundEntry<T> {
		match message.get_message_proof() {
			None => InboundEntry::Message {
				session_id: inbound_processing_info.current_session_id.clone(),
				domain_address: inbound_processing_info.domain_address.clone(),
				message,
				expected_proof_count: inbound_processing_info.expected_proof_count_per_message,
			},
			Some(_) => InboundEntry::Proof {
				session_id: inbound_processing_info.current_session_id,
				current_count: 1,
			},
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
		let router_ids = inbound_processing_info.router_ids.clone();

		ensure!(
			router_ids.iter().any(|x| x == router_id),
			Error::<T>::UnknownRouter
		);

		match inbound_entry {
			InboundEntry::Message { .. } => {
				ensure!(
					router_ids.get(0) == Some(&router_id),
					Error::<T>::MessageExpectedFromFirstRouter
				);

				Ok(())
			}
			InboundEntry::Proof { .. } => {
				ensure!(
					router_ids.get(0) != Some(&router_id),
					Error::<T>::ProofNotExpectedFromFirstRouter
				);

				Ok(())
			}
		}
	}

	/// Upserts an inbound entry for a particular message, increasing the
	/// relevant counts accordingly.
	fn upsert_pending_entry(
		message_proof: Proof,
		router_id: T::RouterId,
		inbound_entry: InboundEntry<T>,
		weight: &mut Weight,
	) -> DispatchResult {
		weight.saturating_accrue(T::DbWeight::get().writes(1));

		PendingInboundEntries::<T>::try_mutate(message_proof, router_id, |storage_entry| {
			match storage_entry {
				None => {
					*storage_entry = Some(inbound_entry);

					Ok::<(), DispatchError>(())
				}
				Some(stored_inbound_entry) => match stored_inbound_entry {
					InboundEntry::Message {
						session_id: stored_session_id,
						expected_proof_count: stored_expected_proof_count,
						..
					} => match inbound_entry {
						InboundEntry::Message {
							session_id: new_session_id,
							expected_proof_count: new_expected_proof_count,
							..
						} => {
							if *stored_session_id != new_session_id {
								*stored_session_id = new_session_id;
								*stored_expected_proof_count = new_expected_proof_count;
							} else {
								stored_expected_proof_count
									.ensure_add_assign(new_expected_proof_count)?;
							}

							Ok::<(), DispatchError>(())
						}
						InboundEntry::Proof { .. } => Err(Error::<T>::ExpectedMessageType.into()),
					},
					InboundEntry::Proof {
						session_id: stored_session_id,
						current_count: stored_current_count,
					} => match inbound_entry {
						InboundEntry::Proof {
							session_id: new_session_id,
							current_count: new_current_count,
						} => {
							if *stored_session_id != new_session_id {
								*stored_session_id = new_session_id;
								*stored_current_count = new_current_count;
							} else {
								stored_current_count.ensure_add_assign(new_current_count)?;
							}

							Ok::<(), DispatchError>(())
						}
						InboundEntry::Message { .. } => {
							Err(Error::<T>::ExpectedMessageProofType.into())
						}
					},
				},
			}
		})
	}

	/// Creates, validates and upserts the inbound entry.
	fn validate_and_upsert_pending_entries(
		inbound_processing_info: &InboundProcessingInfo<T>,
		message: T::Message,
		message_proof: Proof,
		router_id: T::RouterId,
		weight: &mut Weight,
	) -> DispatchResult {
		let inbound_entry = Self::create_inbound_entry(inbound_processing_info, message);

		Self::validate_inbound_entry(&inbound_processing_info, &router_id, &inbound_entry)?;

		Self::upsert_pending_entry(message_proof, router_id, inbound_entry, weight)?;

		Ok(())
	}

	/// Checks if the number of proofs required for executing one message
	/// were received, and if so, decreases the counts accordingly and executes
	/// the message.
	pub(crate) fn execute_if_requirements_are_met(
		inbound_processing_info: &InboundProcessingInfo<T>,
		message_proof: Proof,
		weight: &mut Weight,
	) -> DispatchResult {
		let mut message = None;
		let mut votes = 0;

		for router_id in &inbound_processing_info.router_ids {
			weight.saturating_accrue(T::DbWeight::get().reads(1));

			match PendingInboundEntries::<T>::get(message_proof, router_id) {
				// We expected one InboundEntry for each router, if that's not the case,
				// we can return.
				None => return Ok(()),
				Some(stored_inbound_entry) => match stored_inbound_entry {
					InboundEntry::Message {
						message: stored_message,
						..
					} => message = Some(stored_message),
					InboundEntry::Proof {
						session_id,
						current_count,
					} => {
						if session_id != inbound_processing_info.current_session_id {
							// Don't count vote from invalid sessions.
							continue;
						}

						if current_count > 0 {
							votes += 1;
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
				Self::decrease_pending_entries_counts(
					&inbound_processing_info,
					message_proof,
					weight,
				)?;

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
	fn decrease_pending_entries_counts(
		inbound_processing_info: &InboundProcessingInfo<T>,
		message_proof: Proof,
		weight: &mut Weight,
	) -> DispatchResult {
		for router_id in &inbound_processing_info.router_ids {
			weight.saturating_accrue(T::DbWeight::get().writes(1));

			match PendingInboundEntries::<T>::try_mutate(
				message_proof,
				router_id,
				|storage_entry| match storage_entry {
					None => Err(Error::<T>::PendingInboundEntryNotFound.into()),
					Some(stored_inbound_entry) => {
						match stored_inbound_entry {
							InboundEntry::Message {
								session_id,
								expected_proof_count,
								..
							} => {
								if *session_id != inbound_processing_info.current_session_id {
									// Remove the storage entry completely.
									*storage_entry = None;

									return Ok::<(), DispatchError>(());
								}

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
							InboundEntry::Proof {
								session_id,
								current_count,
							} => {
								if *session_id != inbound_processing_info.current_session_id {
									// Remove the storage entry completely.
									*storage_entry = None;

									return Ok::<(), DispatchError>(());
								}

								let updated_count = (*current_count).ensure_sub(1)?;

								if updated_count == 0 {
									*storage_entry = None;
								} else {
									*current_count = updated_count;
								}

								Ok::<(), DispatchError>(())
							}
						}
					}
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
	pub(crate) fn get_inbound_processing_info(
		domain_address: DomainAddress,
		weight: &mut Weight,
	) -> Result<InboundProcessingInfo<T>, DispatchError> {
		let router_ids = Self::get_router_ids_for_domain(domain_address.domain())?;

		weight.saturating_accrue(T::DbWeight::get().reads(1));

		let current_session_id = SessionIdStore::<T>::get();

		weight.saturating_accrue(T::DbWeight::get().reads(1));

		let expected_proof_count = Self::get_expected_proof_count(&router_ids)?;

		weight.saturating_accrue(T::DbWeight::get().reads(1));

		Ok(InboundProcessingInfo {
			domain_address,
			router_ids,
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
			if let Err(e) = count.ensure_add_assign(1) {
				return (Err(e.into()), weight);
			}

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

			match Self::execute_if_requirements_are_met(
				&inbound_processing_info,
				message_proof,
				&mut weight,
			) {
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
