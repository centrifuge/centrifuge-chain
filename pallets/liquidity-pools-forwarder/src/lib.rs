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
//! Assumptions: The EVM side ensures that incoming forwarded messages are
//! valid.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use core::fmt::Debug;

use cfg_traits::liquidity_pools::{
	LpMessage as LpMessageT, MessageReceiver, MessageSender, RouterProvider,
};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::OriginFor;
pub use pallet::*;
use parity_scale_codec::FullCodec;
use sp_core::H160;
use sp_std::convert::TryInto;

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ForwardInfo {
	/// Refers to the chain from which the message originates.
	///
	/// Example: Assume a three-hop A -> B -> C, then this refers to the domain
	/// of A.
	pub(crate) source_domain: Domain,
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
		type Message: LpMessageT<Domain = Domain>
			+ Clone
			+ Debug
			+ PartialEq
			+ Eq
			+ MaxEncodedLen
			+ TypeInfo
			+ FullCodec;

		/// The entity of the messages coming from this chain.
		type MessageSender: MessageSender<
			Middleware = Self::RouterId,
			Origin = DomainAddress,
			Message = Self::Message,
		>;

		/// The entity which acts on unwrapped messages.
		type MessageReceiver: MessageReceiver<
			Middleware = Self::RouterId,
			Origin = DomainAddress,
			Message = Self::Message,
		>;

		/// An identification of a router.
		type RouterId: Parameter + MaxEncodedLen;

		/// The type that provides the router available for a domain.
		type RouterProvider: RouterProvider<Domain, RouterId = Self::RouterId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Forwarding info was set
		ForwarderSet {
			router_id: T::RouterId,
			source_domain: Domain,
			forwarding_contract: H160,
		},
		/// Forwarding info was removed
		ForwarderRemoved {
			router_id: T::RouterId,
			source_domain: Domain,
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
			source_domain: Domain,
			forwarding_contract: H160,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			RouterForwarding::<T>::insert(
				&router_id,
				ForwardInfo {
					source_domain,
					contract: forwarding_contract,
				},
			);

			Self::deposit_event(Event::<T>::ForwarderSet {
				router_id,
				source_domain,
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
						source_domain: info.source_domain,
						forwarding_contract: info.contract,
					});
				})
				.ok_or(Error::<T>::ForwardInfoNotFound.into())
		}
	}

	impl<T: Config> MessageSender for Pallet<T> {
		type Message = T::Message;
		type Middleware = T::RouterId;
		type Origin = DomainAddress;

		fn send(
			router_id: T::RouterId,
			origin: DomainAddress,
			message: T::Message,
		) -> DispatchResult {
			let msg = RouterForwarding::<T>::get(&router_id)
				.map(|info| {
					T::Message::try_wrap_forward(info.source_domain, info.contract, message.clone())
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
		type Origin = DomainAddress;

		fn receive(
			forwarder_router_id: T::RouterId,
			forwarder_domain_address: DomainAddress,
			message: T::Message,
		) -> DispatchResult {
			// Message can be unwrapped iff it was forwarded
			if let Some((source_domain, _contract, lp_message)) = message.clone().unwrap_forwarded()
			{
				let router_ids = T::RouterProvider::routers_for_domain(source_domain);
				for router_id in router_ids {
					// NOTE: It suffices to check for forwarding existence without need to check the
					// forwarding contract address. For that, we can rely on EVM side to ensure
					// forwarded messages are valid
					if RouterForwarding::<T>::get(&router_id).is_some() {
						return T::MessageReceiver::receive(
							// Since message was sent from forwarder router id, Gateway needs to
							// check whether that id is whitelisted, not the source domain
							forwarder_router_id,
							forwarder_domain_address,
							lp_message,
						);
					}
				}
				Err(Error::<T>::ForwardInfoNotFound.into())
			} else {
				T::MessageReceiver::receive(forwarder_router_id, forwarder_domain_address, message)
			}
		}
	}
}
