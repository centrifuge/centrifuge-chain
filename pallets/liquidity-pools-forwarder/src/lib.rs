// Copyright 2024 Centrifuge Foundation (centrifuge.io).
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
//! # Liquidity Pools Forwarder Pallet.
//!
//! The Forwarder pallet acts as middleware for incoming and outgoing Liquidity
//! Pools messages by wrapping them, if they are forwarded ones.
//!
//! For incoming messages, it extracts the payload from forwarded messages.
//!
//! For outgoing messages, it wraps the payload based on the configured router
//! info.
//!
//! Assumptions:
//!  * The EVM side ensures that incoming forwarded messages are valid.
//!  * Nesting forwarded messages is not allowed, e.g. messages from A are
//!    forwarded exactly via one intermediary domain B to recipient C

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use core::fmt::Debug;

use cfg_traits::liquidity_pools::{LpMessageForwarded, MessageReceiver, MessageSender};
use cfg_types::domain_address::Domain;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::OriginFor;
pub use pallet::*;
use parity_scale_codec::FullCodec;
use sp_core::H160;
use sp_std::convert::TryInto;

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ForwardInfo {
	/// Refers to contract on forwarding chain.
	///
	/// Example: Assume A -> B -> C, then this refers to the forwarding
	/// contract address on B.
	pub(crate) contract: H160,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Required origin for configuring domain forwarding.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The Liquidity Pools message type.
		type Message: LpMessageForwarded<Domain = Domain>
			+ Clone
			+ Debug
			+ PartialEq
			+ Eq
			+ MaxEncodedLen
			+ TypeInfo
			+ FullCodec;

		/// The entity of the messages coming from this chain.
		type MessageSender: MessageSender<Middleware = Self::RouterId, Message = Self::Message>;

		/// The entity which acts on unwrapped messages.
		type MessageReceiver: MessageReceiver<
			Middleware = Self::RouterId,
			Origin = Domain,
			Message = Self::Message,
		>;

		/// An identification of a router.
		type RouterId: Parameter + MaxEncodedLen;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Forwarding info was set
		ForwarderSet {
			router_id: T::RouterId,
			forwarding_contract: H160,
		},
		/// Forwarding info was removed
		ForwarderRemoved {
			router_id: T::RouterId,
			forwarding_contract: H160,
		},
	}

	/// Maps a router id to its forwarding info.
	///
	/// Can only be mutated via admin origin.
	#[pallet::storage]
	pub type RouterForwarding<T: Config> =
		StorageMap<_, Blake2_128Concat, T::RouterId, ForwardInfo, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// The router id does not have any forwarder info stored
		ForwardInfoNotFound,
		/// Failed to unwrap a message which should be a forwarded one
		UnwrappingFailed,
		/// Received a forwarded message from source domain `A` which contradics
		/// the corresponding stored forwarding info that expects source domain
		/// `B`
		///
		/// NOTE: Should never occur because we can assume EVM ensures message
		/// validity
		SourceDomainMismatch,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set forwarding info for the given router id.
		///
		/// Origin: Admin.
		///
		/// NOTE: Simple weight due to origin requirement.
		#[pallet::weight(T::DbWeight::get().writes(1))]
		#[pallet::call_index(0)]
		pub fn set_forwarder(
			origin: OriginFor<T>,
			router_id: T::RouterId,
			forwarding_contract: H160,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			RouterForwarding::<T>::insert(
				&router_id,
				ForwardInfo {
					contract: forwarding_contract,
				},
			);

			Self::deposit_event(Event::<T>::ForwarderSet {
				router_id,
				forwarding_contract,
			});

			Ok(())
		}

		/// Remove the forwarding info for the given router id.
		///
		/// Origin: Admin.
		///
		/// NOTE: Simple weight due to origin requirement.
		#[pallet::weight(T::DbWeight::get().writes(1))]
		#[pallet::call_index(1)]
		pub fn remove_forwarder(origin: OriginFor<T>, router_id: T::RouterId) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			RouterForwarding::<T>::take(&router_id)
				.map(|info| {
					Self::deposit_event(Event::<T>::ForwarderRemoved {
						router_id,
						forwarding_contract: info.contract,
					});
				})
				.ok_or(Error::<T>::ForwardInfoNotFound.into())
		}
	}

	impl<T: Config> MessageSender for Pallet<T> {
		type Message = T::Message;
		type Middleware = T::RouterId;
		type Origin = <T::MessageSender as MessageSender>::Origin;

		fn send(
			router_id: T::RouterId,
			origin: Self::Origin,
			message: T::Message,
		) -> DispatchResult {
			let msg = RouterForwarding::<T>::get(&router_id)
				.map(|info| {
					T::Message::try_wrap_forward(Domain::Centrifuge, info.contract, message.clone())
				})
				.unwrap_or_else(|| {
					ensure!(!message.is_forwarded(), Error::<T>::ForwardInfoNotFound);
					Ok(message)
				})?;

			T::MessageSender::send(router_id, origin, msg)
		}
	}

	impl<T: Config> MessageReceiver for Pallet<T> {
		type Message = T::Message;
		type Middleware = T::RouterId;
		type Origin = Domain;

		fn receive(
			router_id: T::RouterId,
			forwarding_domain: Domain,
			message: T::Message,
		) -> DispatchResult {
			// Message can be unwrapped iff it was forwarded
			let (lp_message, domain) = match (
				RouterForwarding::<T>::get(&router_id).is_some(),
				message.clone().unwrap_forwarded(),
			) {
				// NOTE: Contract address irrelevant here because it is only necessary for
				// outbound forwarded messages
				(true, Some((source_domain, _contract, lp_message))) => {
					Ok((lp_message, source_domain))
				}
				(true, None) => Err(Error::<T>::UnwrappingFailed),
				(false, None) => Ok((message, forwarding_domain)),
				(false, Some((_, _, _))) => Err(Error::<T>::ForwardInfoNotFound),
			}
			.map_err(|e: Error<T>| e)?;

			T::MessageReceiver::receive(router_id, domain, lp_message)
		}
	}
}
