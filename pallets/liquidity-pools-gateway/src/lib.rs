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
	liquidity_pools::{
		InboundQueue, LPEncoding, MessageProcessor, OutboundQueue, Router as DomainRouter,
	},
	TryConvert,
};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{
	dispatch::{DispatchResult, PostDispatchInfo},
	pallet_prelude::*,
	PalletError,
};
use frame_system::{
	ensure_signed,
	pallet_prelude::{BlockNumberFor, OriginFor},
};
use orml_traits::GetByKey;
pub use pallet::*;
use parity_scale_codec::{EncodeLike, FullCodec};
use sp_runtime::traits::{AtLeast32BitUnsigned, EnsureAdd, EnsureAddAssign, One};
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

	use frame_support::dispatch::PostDispatchInfo;
	use sp_runtime::DispatchErrorWithPostInfo;

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

		/// The incoming and outgoing message type.
		///
		/// NOTE - this `Codec` trait is the Centrifuge trait for liquidity
		/// pools' messages.
		type Message: LPEncoding + Clone + Debug + PartialEq + MaxEncodedLen + TypeInfo + FullCodec;

		/// The message router type that is stored for each domain.
		type Router: DomainRouter<Sender = Self::AccountId>
			+ Clone
			+ Debug
			+ MaxEncodedLen
			+ TypeInfo
			+ FullCodec
			+ EncodeLike
			+ PartialEq;

		/// The type that processes incoming messages.
		type InboundQueue: InboundQueue<Sender = DomainAddress, Message = Self::Message>;

		/// A way to recover a domain address from two byte slices
		type OriginRecovery: TryConvert<(Vec<u8>, Vec<u8>), DomainAddress, Error = DispatchError>;

		type WeightInfo: WeightInfo;

		/// Maximum size of an incoming message.
		#[pallet::constant]
		type MaxIncomingMessageSize: Get<u32>;

		/// The sender account that will be used in the OutboundQueue
		/// implementation.
		#[pallet::constant]
		type Sender: Get<Self::AccountId>;

		/// Type used for outbound message identification.
		type OutboundMessageNonce: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ EnsureAdd
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;
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

		/// An outbound message has been submitted.
		OutboundMessageSubmitted {
			sender: T::AccountId,
			domain: Domain,
			message: T::Message,
		},

		/// Outbound message execution failure.
		OutboundMessageExecutionFailure {
			nonce: T::OutboundMessageNonce,
			sender: T::AccountId,
			domain: Domain,
			message: T::Message,
			error: DispatchError,
		},

		/// Outbound message execution success.
		OutboundMessageExecutionSuccess {
			nonce: T::OutboundMessageNonce,
			sender: T::AccountId,
			domain: Domain,
			message: T::Message,
		},

		/// The domain hook address was initialized or updated.
		DomainHookAddressSet {
			domain: Domain,
			hook_address: [u8; 20],
		},
	}

	/// Storage for domain routers.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	#[pallet::getter(fn domain_routers)]
	pub type DomainRouters<T: Config> = StorageMap<_, Blake2_128Concat, Domain, T::Router>;

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

	#[pallet::storage]
	#[pallet::getter(fn outbound_message_nonce_store)]
	pub type OutboundMessageNonceStore<T: Config> =
		StorageValue<_, T::OutboundMessageNonce, ValueQuery>;

	/// Storage for outbound messages that will be processed during the
	/// `on_idle` hook.
	#[pallet::storage]
	#[pallet::getter(fn outbound_message_queue)]
	pub type OutboundMessageQueue<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::OutboundMessageNonce,
		(Domain, T::AccountId, T::Message),
	>;

	/// Storage for failed outbound messages that can be manually re-triggered.
	#[pallet::storage]
	#[pallet::getter(fn failed_outbound_messages)]
	pub type FailedOutboundMessages<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::OutboundMessageNonce,
		(Domain, T::AccountId, T::Message, DispatchError),
	>;

	/// Stores the hook address of a domain required for particular LP messages.
	///
	/// Lifetime: Indefinitely.
	///
	/// NOTE: Must only be changeable via root or `AdminOrigin`.
	#[pallet::storage]
	pub type DomainHookAddress<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, [u8; 20], OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Router initialization failed.
		RouterInitFailed,

		/// The origin of the message to be processed is invalid.
		InvalidMessageOrigin,

		/// The domain is not supported.
		DomainNotSupported,

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

		/// Relayer messages need to prepend the with
		/// the original source chain and source address
		/// that triggered the message.
		/// Decoding that is essential and this error
		/// signals malforming of the wrapping information.
		RelayerMessageDecodingFailed { reason: RelayerMessageDecodingError },

		/// Outbound message not found in storage.
		OutboundMessageNotFound,

		/// Failed outbound message not found in storage.
		FailedOutboundMessageNotFound,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_idle(_now: BlockNumberFor<T>, max_weight: Weight) -> Weight {
			Self::service_outbound_message_queue(max_weight)
		}
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
			let (domain_address, incoming_msg) = match T::LocalEVMOrigin::ensure_origin(origin)? {
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

			T::InboundQueue::submit(domain_address, incoming_msg)
		}

		/// Convenience method for manually processing an outbound message.
		///
		/// If the execution fails, the message gets moved to the
		/// `FailedOutboundMessages` storage.
		#[pallet::weight(T::WeightInfo::process_outbound_message())]
		#[pallet::call_index(6)]
		pub fn process_outbound_message(
			origin: OriginFor<T>,
			nonce: T::OutboundMessageNonce,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let (domain, sender, message) = OutboundMessageQueue::<T>::take(nonce)
				.ok_or(Error::<T>::OutboundMessageNotFound)?;

			match Self::process_message(domain.clone(), sender.clone(), message.clone()) {
				Ok(_) => {
					Self::deposit_event(Event::<T>::OutboundMessageExecutionSuccess {
						nonce,
						domain,
						sender,
						message,
					});

					Ok(())
				}
				Err(e) => {
					Self::deposit_event(Event::<T>::OutboundMessageExecutionFailure {
						nonce,
						domain: domain.clone(),
						sender: sender.clone(),
						message: message.clone(),
						error: e.error,
					});

					FailedOutboundMessages::<T>::insert(nonce, (domain, sender, message, e.error));

					Ok(())
				}
			}
		}

		/// Manually process a failed outbound message.
		#[pallet::weight(T::WeightInfo::process_failed_outbound_message())]
		#[pallet::call_index(7)]
		pub fn process_failed_outbound_message(
			origin: OriginFor<T>,
			nonce: T::OutboundMessageNonce,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let (domain, sender, message, _) = FailedOutboundMessages::<T>::get(nonce)
				.ok_or(Error::<T>::OutboundMessageNotFound)?;

			match Self::process_message(domain.clone(), sender.clone(), message.clone()) {
				Ok(_) => {
					Self::deposit_event(Event::<T>::OutboundMessageExecutionSuccess {
						nonce,
						domain,
						sender,
						message,
					});

					FailedOutboundMessages::<T>::remove(nonce);

					Ok(())
				}
				Err(e) => {
					Self::deposit_event(Event::<T>::OutboundMessageExecutionFailure {
						nonce,
						domain: domain.clone(),
						sender: sender.clone(),
						message: message.clone(),
						error: e.error,
					});

					Ok(())
				}
			}
		}

		/// Set the address of the domain hook
		///
		/// Can only be called by `AdminOrigin`.
		#[pallet::weight(T::WeightInfo::set_domain_router())]
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
		) -> Result<(DomainAddress, T::Message), DispatchError> {
			if let DomainAddress::Centrifuge(_) = address {
				return Err(Error::<T>::InvalidMessageOrigin.into());
			}

			ensure!(
				Allowlist::<T>::contains_key(address.domain(), address.clone()),
				Error::<T>::UnknownInstance,
			);

			let incoming_msg = T::Message::deserialize(msg.as_slice())
				.map_err(|_| Error::<T>::MessageDecodingFailed)?;

			Ok((address, incoming_msg))
		}

		/// Iterates over the outbound messages stored in the queue and attempts
		/// to process them.
		///
		/// If a message fails to process it is moved to the
		/// `FailedOutboundMessages` storage so that it can be executed again
		/// via the `process_failed_outbound_message` extrinsic.
		fn service_outbound_message_queue(max_weight: Weight) -> Weight {
			let mut weight_used = Weight::zero();

			let mut processed_entries = Vec::new();

			for (nonce, (domain, sender, message)) in OutboundMessageQueue::<T>::iter() {
				processed_entries.push(nonce);

				let weight =
					match Self::process_message(domain.clone(), sender.clone(), message.clone()) {
						Ok(post_info) => {
							Self::deposit_event(Event::OutboundMessageExecutionSuccess {
								nonce,
								sender,
								domain,
								message,
							});

							post_info
								.actual_weight
								.expect("Message processing success already ensured")
								// Extra weight breakdown:
								//
								// 1 read for the outbound message
								// 1 write for the event
								// 1 write for the outbound message removal
								.saturating_add(T::DbWeight::get().reads_writes(1, 2))
						}
						Err(e) => {
							Self::deposit_event(Event::OutboundMessageExecutionFailure {
								nonce,
								sender: sender.clone(),
								domain: domain.clone(),
								message: message.clone(),
								error: e.error,
							});

							FailedOutboundMessages::<T>::insert(
								nonce,
								(domain, sender, message, e.error),
							);

							e.post_info
								.actual_weight
								.expect("Message processing success already ensured")
								// Extra weight breakdown:
								//
								// 1 read for the outbound message
								// 1 write for the event
								// 1 write for the failed outbound message
								// 1 write for the outbound message removal
								.saturating_add(T::DbWeight::get().reads_writes(1, 3))
						}
					};

				weight_used = weight_used.saturating_add(weight);

				if weight_used.all_gte(max_weight) {
					break;
				}
			}

			for entry in processed_entries {
				OutboundMessageQueue::<T>::remove(entry);
			}

			weight_used
		}

		/// Retrieves the router stored for the provided domain and sends the
		/// message, calculating and returning the required weight for these
		/// operations in the `DispatchResultWithPostInfo`.
		fn process_message(
			domain: Domain,
			sender: T::AccountId,
			message: T::Message,
		) -> DispatchResultWithPostInfo {
			let read_weight = T::DbWeight::get().reads(1);

			let router = DomainRouters::<T>::get(domain).ok_or(DispatchErrorWithPostInfo {
				post_info: PostDispatchInfo {
					actual_weight: Some(read_weight),
					pays_fee: Pays::Yes,
				},
				error: Error::<T>::RouterNotFound.into(),
			})?;

			let post_dispatch_info_fn =
				|actual_weight: Option<Weight>, extra_weight: Weight| -> PostDispatchInfo {
					PostDispatchInfo {
						actual_weight: Some(Self::get_outbound_message_processing_weight(
							actual_weight,
							extra_weight,
						)),
						pays_fee: Pays::Yes,
					}
				};

			match router.send(sender, message.serialize()) {
				Ok(dispatch_info) => Ok(post_dispatch_info_fn(
					dispatch_info.actual_weight,
					read_weight,
				)),
				Err(e) => Err(DispatchErrorWithPostInfo {
					post_info: post_dispatch_info_fn(e.post_info.actual_weight, read_weight),
					error: e.error,
				}),
			}
		}

		/// Calculates the weight used by a router when processing an outbound
		/// message.
		fn get_outbound_message_processing_weight(
			router_call_weight: Option<Weight>,
			extra_weight: Weight,
		) -> Weight {
			let pov_weight: u64 = (Domain::max_encoded_len()
				+ T::AccountId::max_encoded_len()
				+ T::Message::max_encoded_len())
			.try_into()
			.expect("can calculate outbound message POV weight");

			router_call_weight
				.unwrap_or(Weight::from_parts(DEFAULT_WEIGHT_REF_TIME, 0))
				.saturating_add(Weight::from_parts(0, pov_weight))
				.saturating_add(extra_weight)
		}
	}

	/// This pallet will be the `OutboundQueue` used by other pallets to send
	/// outgoing messages.
	///
	/// NOTE - the sender provided as an argument is not used at the moment, we
	/// are using the sender specified in the pallet config so that we can
	/// ensure that the account is funded.
	impl<T: Config> OutboundQueue for Pallet<T> {
		type Destination = Domain;
		type Message = T::Message;
		type Sender = T::AccountId;

		fn submit(
			_sender: Self::Sender,
			destination: Self::Destination,
			message: Self::Message,
		) -> DispatchResult {
			ensure!(
				destination != Domain::Centrifuge,
				Error::<T>::DomainNotSupported
			);

			ensure!(
				DomainRouters::<T>::contains_key(destination.clone()),
				Error::<T>::RouterNotFound
			);

			let nonce = <OutboundMessageNonceStore<T>>::try_mutate(|n| {
				n.ensure_add_assign(T::OutboundMessageNonce::one())?;
				Ok::<T::OutboundMessageNonce, DispatchError>(*n)
			})?;

			OutboundMessageQueue::<T>::insert(
				nonce,
				(destination.clone(), T::Sender::get(), message.clone()),
			);

			Self::deposit_event(Event::OutboundMessageSubmitted {
				sender: T::Sender::get(),
				domain: destination,
				message,
			});

			Ok(())
		}
	}
}

impl<T: Config> GetByKey<Domain, Option<[u8; 20]>> for Pallet<T> {
	fn get(domain: &Domain) -> Option<[u8; 20]> {
		DomainHookAddress::<T>::get(domain)
	}
}

impl<T: Config> MessageProcessor for Pallet<T> {
	type Message = T::Message;

	fn process(_: Self::Message) -> DispatchResultWithPostInfo {
		Ok(PostDispatchInfo::default())
	}
}
