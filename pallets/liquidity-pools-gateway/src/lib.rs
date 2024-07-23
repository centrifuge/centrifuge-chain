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

use cfg_traits::{
	liquidity_pools::{InboundMessageHandler, LPMessage, Router as DomainRouter},
	TryConvert,
};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, PalletError};
use frame_system::pallet_prelude::OriginFor;
pub use pallet::*;
use parity_scale_codec::{EncodeLike, FullCodec};
use sp_std::{convert::TryInto, vec::Vec};

use crate::weights::WeightInfo;

mod origin;
pub use origin::*;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum RelayerMessageDecodingError {
	MalformedSourceAddress,
	MalformedSourceAddressLength,
	MalformedSourceChain,
	MalformedSourceChainLength,
	MalformedMessage,
}

impl<T: Config> From<RelayerMessageDecodingError> for Error<T> {
	fn from(value: RelayerMessageDecodingError) -> Self {
		Error::RelayerMessageDecodingFailed { reason: value }
	}
}

#[frame_support::pallet]
pub mod pallet {
	const BYTES_U32: usize = 4;
	const BYTES_ACCOUNT_20: usize = 20;

	/// Some gateway routers do not return an actual weight when sending a
	/// message, thus, this default is required, and it's based on:
	///
	/// https://github.com/centrifuge/centrifuge-chain/pull/1696#discussion_r1456370592
	const DEFAULT_WEIGHT_REF_TIME: u64 = 5_000_000_000;

	use cfg_traits::liquidity_pools::{LPEncoding, MessageProcessor, MessageQueue};
	use cfg_types::gateway::GatewayMessage;
	use frame_support::dispatch::PostDispatchInfo;
	use sp_runtime::{traits::EnsureSub, DispatchErrorWithPostInfo};

	use super::*;
	use crate::RelayerMessageDecodingError::{
		MalformedMessage, MalformedSourceAddress, MalformedSourceAddressLength,
		MalformedSourceChain, MalformedSourceChainLength,
	};

	#[pallet::pallet]

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

		/// The LP message type.
		type LPMessage: LPMessage + Clone + Debug + PartialEq + MaxEncodedLen + TypeInfo + FullCodec;

		/// The message router type that is stored for each domain.
		type Router: DomainRouter<Sender = Self::AccountId>
			+ Clone
			+ Debug
			+ MaxEncodedLen
			+ TypeInfo
			+ FullCodec
			+ EncodeLike
			+ PartialEq;

		/// The type that handles incoming messages.
		type InboundMessageHandler: InboundMessageHandler<
			Sender = DomainAddress,
			Message = Self::LPMessage,
		>;

		/// A way to recover a domain address from two byte slices.
		type OriginRecovery: TryConvert<(Vec<u8>, Vec<u8>), DomainAddress, Error = DispatchError>;

		type WeightInfo: WeightInfo;

		/// Maximum size of an incoming message.
		#[pallet::constant]
		type MaxIncomingMessageSize: Get<u32>;

		/// The sender account that will be used in the OutboundQueue
		/// implementation.
		#[pallet::constant]
		type Sender: Get<Self::AccountId>;

		/// Maximum number of routers allowed for a domain in a multi-router
		/// setup.
		#[pallet::constant]
		type MaxRouterCount: Get<u32>;

		/// Type used for queueing messages.
		type MessageQueue: MessageQueue<Message = GatewayMessage<Self::AccountId, Self::LPMessage>>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The router for a given domain was set.
		DomainRouterSet { domain: Domain, router: T::Router },

		/// An instance was added to a domain.
		InstanceAdded { instance: DomainAddress },

		/// An instance was removed from a domain.
		InstanceRemoved { instance: DomainAddress },

		/// A relayer was added.
		RelayerAdded { relayer: DomainAddress },

		/// A relayer was removed.
		RelayerRemoved { relayer: DomainAddress },

		/// The router for a given domain was set.
		DomainMultiRouterSet {
			domain: Domain,
			routers: BoundedVec<T::Router, T::MaxRouterCount>,
		},
	}

	/// Storage for domain routers.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	#[pallet::getter(fn domain_routers)]
	pub type DomainRouters<T: Config> = StorageMap<_, Blake2_128Concat, Domain, T::Router>;

	/// Storage for domain multi-routers.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	#[pallet::getter(fn domain_multi_routers)]
	pub type DomainMultiRouters<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, BoundedVec<T::Router, T::MaxRouterCount>>;

	/// Storage that contains a limited number of whitelisted instances of
	/// deployed liquidity pools for a particular domain.
	///
	/// This can only be modified by an admin.
	#[pallet::storage]
	#[pallet::getter(fn allowlist)]
	pub type Allowlist<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, Domain, Blake2_128Concat, DomainAddress, ()>;

	/// Storage that contains a limited number of whitelisted instances of
	/// deployed liquidity pools for a particular domain.
	///
	/// This can only be modified by an admin.
	#[pallet::storage]
	#[pallet::getter(fn relayer)]
	pub type RelayerList<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, Domain, Blake2_128Concat, DomainAddress, ()>;

	/// Storage that keeps track of incoming message proofs.
	#[pallet::storage]
	#[pallet::getter(fn inbound_message_proof_count)]
	pub type InboundMessageProofCount<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 32], u32, ValueQuery>;

	/// Storage that keeps track of incoming messages.
	#[pallet::storage]
	#[pallet::getter(fn inbound_messages)]
	pub type InboundMessages<T: Config> = StorageMap<_, Blake2_128Concat, [u8; 32], T::LPMessage>;

	#[pallet::error]
	pub enum Error<T> {
		/// Router initialization failed.
		RouterInitFailed,

		/// The origin of the message to be processed is invalid.
		InvalidMessageOrigin,

		/// The domain is not supported.
		DomainNotSupported,

		/// Invalid multi router.
		InvalidMultiRouter,

		/// Message decoding error.
		MessageDecodingFailed,

		/// Instance was already added to the domain.
		InstanceAlreadyAdded,

		/// Relayer was already added to the domain
		RelayerAlreadyAdded,

		/// Maximum number of instances for a domain was reached.
		MaxDomainInstances,

		/// Unknown instance.
		UnknownInstance,

		/// Unknown relayer
		UnknownRelayer,

		/// Router not found.
		RouterNotFound,

		/// Multi-router not found.
		MultiRouterNotFound,

		/// Relayer messages need to prepend the with
		/// the original source chain and source address
		/// that triggered the message.
		/// Decoding that is essential and this error
		/// signals malforming of the wrapping information.
		RelayerMessageDecodingFailed { reason: RelayerMessageDecodingError },

		/// Message proof cannot be retrieved.
		MessageProofRetrieval,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set a domain's router,
		#[pallet::weight(T::WeightInfo::set_domain_router())]
		#[pallet::call_index(0)]
		pub fn set_domain_router(
			origin: OriginFor<T>,
			domain: Domain,
			router: T::Router,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			ensure!(domain != Domain::Centrifuge, Error::<T>::DomainNotSupported);

			router.init().map_err(|_| Error::<T>::RouterInitFailed)?;

			<DomainRouters<T>>::insert(domain.clone(), router.clone());

			Self::deposit_event(Event::DomainRouterSet { domain, router });

			Ok(())
		}

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

		/// Add a known instance of a deployed liquidity pools integration for a
		/// specific domain.
		#[pallet::weight(T::WeightInfo::add_relayer())]
		#[pallet::call_index(3)]
		pub fn add_relayer(origin: OriginFor<T>, relayer: DomainAddress) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			ensure!(
				relayer.domain() != Domain::Centrifuge,
				Error::<T>::DomainNotSupported
			);

			ensure!(
				!RelayerList::<T>::contains_key(relayer.domain(), relayer.clone()),
				Error::<T>::RelayerAlreadyAdded,
			);

			RelayerList::<T>::insert(relayer.domain(), relayer.clone(), ());

			Self::deposit_event(Event::RelayerAdded { relayer });

			Ok(())
		}

		/// Remove an instance from a specific domain.
		#[pallet::weight(T::WeightInfo::remove_relayer())]
		#[pallet::call_index(4)]
		pub fn remove_relayer(origin: OriginFor<T>, relayer: DomainAddress) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			ensure!(
				RelayerList::<T>::contains_key(relayer.domain(), relayer.clone()),
				Error::<T>::UnknownRelayer,
			);

			RelayerList::<T>::remove(relayer.domain(), relayer.clone());

			Self::deposit_event(Event::RelayerRemoved { relayer });

			Ok(())
		}

		/// Process an incoming message.
		#[pallet::weight(T::WeightInfo::process_msg())]
		#[pallet::call_index(5)]
		pub fn process_msg(
			origin: OriginFor<T>,
			msg: BoundedVec<u8, T::MaxIncomingMessageSize>,
		) -> DispatchResult {
			let (domain_address, message) = match T::LocalEVMOrigin::ensure_origin(origin)? {
				GatewayOrigin::Domain(domain_address) => {
					Pallet::<T>::validate(domain_address, msg)?
				}
				GatewayOrigin::AxelarRelay(domain_address) => {
					// Every axelar relay address has a separate storage
					ensure!(
						RelayerList::<T>::contains_key(domain_address.domain(), domain_address),
						Error::<T>::UnknownRelayer
					);

					// Every axelar relay will prepend the (sourceChain,
					// sourceAddress) from actual origination chain to the
					// message bytes, with a length identifier
					let slice_ref = &mut msg.as_slice();
					let length_source_chain: usize = Pallet::<T>::try_range(
						slice_ref,
						BYTES_U32,
						Error::<T>::from(MalformedSourceChainLength),
						|be_bytes_u32| {
							let mut bytes = [0u8; BYTES_U32];
							// NOTE: This can NEVER panic as the `try_range` logic ensures the given
							// bytes have the right length. I.e. 4 in this case
							bytes.copy_from_slice(be_bytes_u32);

							u32::from_be_bytes(bytes).try_into().map_err(|_| {
								DispatchError::Other("Expect: usize in wasm is always ge u32")
							})
						},
					)?;

					let source_chain = Pallet::<T>::try_range(
						slice_ref,
						length_source_chain,
						Error::<T>::from(MalformedSourceChain),
						|source_chain| Ok(source_chain.to_vec()),
					)?;

					let length_source_address: usize = Pallet::<T>::try_range(
						slice_ref,
						BYTES_U32,
						Error::<T>::from(MalformedSourceAddressLength),
						|be_bytes_u32| {
							let mut bytes = [0u8; BYTES_U32];
							// NOTE: This can NEVER panic as the `try_range` logic ensures the given
							// bytes have the right length. I.e. 4 in this case
							bytes.copy_from_slice(be_bytes_u32);

							u32::from_be_bytes(bytes).try_into().map_err(|_| {
								DispatchError::Other("Expect: usize in wasm is always ge u32")
							})
						},
					)?;

					let source_address = Pallet::<T>::try_range(
						slice_ref,
						length_source_address,
						Error::<T>::from(MalformedSourceAddress),
						|source_address| {
							// NOTE: Axelar simply provides the hexadecimal string of an EVM
							//       address as the `sourceAddress` argument. Solidity does on the
							//       other side recognize the hex-encoding and encode the hex bytes
							//       to utf-8 bytes.
							//
							//       Hence, we are reverting this process here.
							let source_address =
								cfg_utils::decode_var_source::<BYTES_ACCOUNT_20>(source_address)
									.ok_or(Error::<T>::from(MalformedSourceAddress))?;

							Ok(source_address.to_vec())
						},
					)?;

					let origin_msg = Pallet::<T>::try_range(
						slice_ref,
						slice_ref.len(),
						Error::<T>::from(MalformedMessage),
						|msg| {
							BoundedVec::try_from(msg.to_vec()).map_err(|_| {
								DispatchError::Other(
									"Remaining bytes smaller vector in the first place. qed.",
								)
							})
						},
					)?;

					let origin_domain =
						T::OriginRecovery::try_convert((source_chain, source_address))?;

					Pallet::<T>::validate(origin_domain, origin_msg)?
				}
			};

			let gateway_message = GatewayMessage::<T::AccountId, T::LPMessage>::Inbound {
				domain_address,
				message,
			};

			T::MessageQueue::submit(gateway_message)
		}

		/// Set routers for a particular domain.
		#[pallet::weight(T::WeightInfo::set_domain_multi_router())]
		#[pallet::call_index(8)]
		pub fn set_domain_multi_router(
			origin: OriginFor<T>,
			domain: Domain,
			routers: BoundedVec<T::Router, T::MaxRouterCount>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			ensure!(domain != Domain::Centrifuge, Error::<T>::DomainNotSupported);
			ensure!(routers.len() > 0, Error::<T>::InvalidMultiRouter);

			for router in &routers {
				router.init().map_err(|_| Error::<T>::RouterInitFailed)?;
			}

			<DomainMultiRouters<T>>::insert(domain.clone(), routers.clone());

			Self::deposit_event(Event::DomainMultiRouterSet { domain, routers });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn try_range<'a, D, F>(
			slice: &mut &'a [u8],
			next_steps: usize,
			error: Error<T>,
			transformer: F,
		) -> Result<D, DispatchError>
		where
			F: Fn(&'a [u8]) -> Result<D, DispatchError>,
		{
			ensure!(slice.len() >= next_steps, error);

			let (input, new_slice) = slice.split_at(next_steps);
			let res = transformer(input)?;
			*slice = new_slice;

			Ok(res)
		}

		fn validate(
			address: DomainAddress,
			msg: BoundedVec<u8, T::MaxIncomingMessageSize>,
		) -> Result<(DomainAddress, T::LPMessage), DispatchError> {
			if let DomainAddress::Centrifuge(_) = address {
				return Err(Error::<T>::InvalidMessageOrigin.into());
			}

			ensure!(
				Allowlist::<T>::contains_key(address.domain(), address.clone()),
				Error::<T>::UnknownInstance,
			);

			let incoming_msg = T::LPMessage::deserialize(msg.as_slice())
				.map_err(|_| Error::<T>::MessageDecodingFailed)?;

			Ok((address, incoming_msg))
		}

		fn process_inbound_message(
			domain_address: DomainAddress,
			message: T::LPMessage,
		) -> DispatchResultWithPostInfo {
			let mut post_info = PostDispatchInfo {
				actual_weight: Some(T::DbWeight::get().reads(1)),
				pays_fee: Pays::Yes,
			};

			let routers_count = DomainMultiRouters::<T>::get(domain_address.domain())
				.ok_or(DispatchErrorWithPostInfo {
					post_info,
					error: Error::<T>::MultiRouterNotFound.into(),
				})?
				.len();

			let expected_proof_count = routers_count.ensure_sub(1)?;

			let (message_proof, message_proof_count) = match message.get_message_proof() {
				None => {
					let message_proof = message
						.to_message_proof()
						.get_message_proof()
						.ok_or(Error::<T>::MessageProofRetrieval)?;

					InboundMessages::<T>::insert(message_proof, message);

					(
						message_proof,
						InboundMessageProofCount::<T>::get(message_proof),
					)
				}
				Some(message_proof) => {
					let message_proof_count =
						InboundMessageProofCount::<T>::try_mutate(message_proof, |count| {
							*count += 1;

							Ok::<u32, DispatchError>(*count)
						})?;

					(message_proof, message_proof_count)
				}
			};

			post_info.actual_weight = Some(
				post_info
					.actual_weight
					.unwrap_or_default()
					.saturating_add(T::DbWeight::get().reads_writes(1, 1)),
			);

			// IMPORTANT - the number of routers on Centrifuge Chain and other domains are
			// the same. It is always expected that one router sends the message and the
			// others are sending the proofs, i.e. if we have 3 routers, we expect 1 message
			// and 2 message proofs.
			if message_proof_count != expected_proof_count as u32 {
				return Ok(post_info);
			}

			post_info.actual_weight = Some(
				post_info
					.actual_weight
					.unwrap_or_default()
					.saturating_add(T::DbWeight::get().reads(1)),
			);

			let message = match InboundMessages::<T>::get(message_proof) {
				Some(m) => m,
				// Not finding the message here is not a problem. We might have the correct message proof
				// count but no actual message.
				None => return Ok(post_info),
			};

			InboundMessages::<T>::remove(message_proof);
			InboundMessageProofCount::<T>::remove(message_proof);

			post_info.actual_weight = Some(
				post_info
					.actual_weight
					.unwrap_or_default()
					.saturating_add(T::DbWeight::get().writes(2)),
			);

			match T::InboundMessageHandler::handle(domain_address, message) {
				Ok(_) => Ok(post_info),
				Err(e) => Err(DispatchErrorWithPostInfo {
					post_info,
					error: e,
				}),
			}
		}

		/// Retrieves the routers stored for the provided domain and sends the
		/// message and message proofs, calculating and returning the required
		/// weight for these operations in the `DispatchResultWithPostInfo`.
		fn process_outbound_message(
			domain: Domain,
			sender: T::AccountId,
			message: T::LPMessage,
		) -> DispatchResultWithPostInfo {
			ensure!(domain != Domain::Centrifuge, Error::<T>::DomainNotSupported);

			let mut post_info = PostDispatchInfo {
				actual_weight: Some(T::DbWeight::get().reads(1)),
				pays_fee: Pays::Yes,
			};

			let routers =
				DomainMultiRouters::<T>::get(domain).ok_or(DispatchErrorWithPostInfo {
					post_info,
					error: Error::<T>::MultiRouterNotFound.into(),
				})?;

			let message_proof = message.to_message_proof();
			let mut message_opt = Some(message);

			for router in routers {
				// Ensure that we only send the actual message once, using one router.
				// The remaining routers will send the message proof.
				let router_msg = match message_opt.take() {
					Some(m) => m.serialize(),
					None => message_proof.serialize(),
				};

				match router.send(sender.clone(), router_msg) {
					Ok(dispatch_info) => Self::update_total_post_dispatch_info_weight(
						&mut post_info,
						dispatch_info.actual_weight,
					),
					Err(e) => {
						Self::update_total_post_dispatch_info_weight(
							&mut post_info,
							e.post_info.actual_weight,
						);

						return Err(DispatchErrorWithPostInfo {
							post_info,
							error: e.error,
						});
					}
				}
			}

			Ok(post_info)
		}

		fn update_total_post_dispatch_info_weight(
			post_dispatch_info: &mut PostDispatchInfo,
			router_call_weight: Option<Weight>,
		) {
			let router_call_weight =
				Self::get_outbound_message_processing_weight(router_call_weight);

			post_dispatch_info.actual_weight = Some(
				post_dispatch_info
					.actual_weight
					.unwrap_or_default()
					.saturating_add(router_call_weight),
			);
		}

		/// Calculates the weight used by a router when processing an outbound
		/// message.
		fn get_outbound_message_processing_weight(router_call_weight: Option<Weight>) -> Weight {
			let pov_weight: u64 = (Domain::max_encoded_len()
				+ T::AccountId::max_encoded_len()
				+ T::LPMessage::max_encoded_len())
			.try_into()
			.expect("can calculate outbound message POV weight");

			router_call_weight
				.unwrap_or(Weight::from_parts(DEFAULT_WEIGHT_REF_TIME, 0))
				.saturating_add(Weight::from_parts(0, pov_weight))
		}
	}

	impl<T: Config> MessageProcessor for Pallet<T> {
		type Message = GatewayMessage<T::AccountId, T::LPMessage>;

		fn process(message: Self::Message) -> DispatchResultWithPostInfo {
			match message {
				GatewayMessage::Inbound {
					domain_address,
					message,
				} => Self::process_inbound_message(domain_address, message),
				GatewayMessage::Outbound {
					destination,
					message,
					..
				} => {
					// Make sure we use the gateway sender.
					let sender = T::Sender::get();

					Self::process_outbound_message(destination, sender, message)
				}
			}
		}
	}
}
