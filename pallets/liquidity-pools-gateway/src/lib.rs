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
#![cfg_attr(not(feature = "std"), no_std)]

use core::fmt::Debug;

use cfg_traits::liquidity_pools::{Codec, InboundQueue, OutboundQueue, Router as DomainRouter};
use cfg_types::domain_address::{Domain, DomainAddress};
use codec::{EncodeLike, FullCodec};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::OriginFor;
pub use pallet::*;
use sp_std::convert::TryInto;

use crate::weights::WeightInfo;

mod origin;
pub use origin::*;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
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
		type Message: Codec;

		/// The message router type that is stored for each domain.
		type Router: DomainRouter<Sender = Self::AccountId, Message = Self::Message>
			+ Clone
			+ Debug
			+ MaxEncodedLen
			+ TypeInfo
			+ FullCodec
			+ EncodeLike
			+ PartialEq;

		/// The type that processes incoming messages.
		type InboundQueue: InboundQueue<Sender = DomainAddress, Message = Self::Message>;

		type WeightInfo: WeightInfo;

		/// Maximum size of an incoming message.
		#[pallet::constant]
		type MaxIncomingMessageSize: Get<u32>;
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
	}

	/// Storage for domain routers.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	pub(crate) type DomainRouters<T: Config> = StorageMap<_, Blake2_128Concat, Domain, T::Router>;

	/// Storage that contains a limited number of whitelisted instances of
	/// deployed liquidity pools for a particular domain.
	///
	/// This can only be modified by an admin.
	#[pallet::storage]
	#[pallet::getter(fn allowlist)]
	pub(crate) type Allowlist<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, Domain, Blake2_128Concat, DomainAddress, ()>;

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

		/// Maximum number of instances for a domain was reached.
		MaxDomainInstances,

		/// Unknown instance.
		UnknownInstance,

		/// Router not found.
		RouterNotFound,
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

		/// Remove a instance from a specific domain.
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

		/// Process an incoming message.
		#[pallet::weight(0)]
		#[pallet::call_index(3)]
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

					// Every axelar relay will prepend the (sourceChain,
					// sourceAddress) from actual origination chain to the
					// message bytes, with a length identifier
					let slice = msg.as_slice();
					let len_source_chain = slice[0..4];
					let source_chain =
					let origin_domain = T::OriginRecovery::try_convert()
						.map_err(|_| Error::<T>::InvalidMessageOrigin)?;

					Pallet::<T>::validate(origin_domain, origin_msg)
				}
			};

			T::InboundQueue::submit(domain_address, incoming_msg)
		}
	}

	impl<T: Config> Pallet<T> {
		fn try_range<D>() -> Result<D, DispatchError> {

		}

		fn validate(
			address: DomainAddress,
			msg: BoundedVec<u8, T::MaxIncomingMessageSize>,
		) -> Result<(DomainAddress, T::Message), DispatchError> {
			ensure!(
				Allowlist::<T>::contains_key(domain_address.domain(), domain_address.clone()),
				Error::<T>::UnknownInstance,
			);

			ensure!(
				domain_address != DomainAddress::Centrifuge(_),
				Error::<T>::InvalidMessageOrigin
			);

			let incoming_msg = T::Message::deserialize(&mut msg.as_slice())
				.map_err(|_| Error::<T>::MessageDecodingFailed)?;

			Ok((domain_address, incoming_msg))
		}
	}

	/// This pallet will be the `OutboundQueue` used by other pallets to send
	/// outgoing messages.
	impl<T: Config> OutboundQueue for Pallet<T> {
		type Destination = Domain;
		type Message = T::Message;
		type Sender = T::AccountId;

		fn submit(
			sender: Self::Sender,
			destination: Self::Destination,
			msg: Self::Message,
		) -> DispatchResult {
			ensure!(
				destination != Domain::Centrifuge,
				Error::<T>::DomainNotSupported
			);

			let router = DomainRouters::<T>::get(destination).ok_or(Error::<T>::RouterNotFound)?;

			router.send(sender, msg)
		}
	}
}
