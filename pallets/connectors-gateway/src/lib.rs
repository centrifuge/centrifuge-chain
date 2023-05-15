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

pub use pallet::*;

mod origin;
pub use origin::*;

pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	use core::fmt::Debug;

	use cfg_traits::connectors::{Codec, InboundQueue, OutboundQueue, Router as DomainRouter};
	use cfg_types::domain_address::{Domain, DomainAddress};
	use codec::{EncodeLike, FullCodec};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;
	use sp_std::{convert::TryInto, vec::Vec};

	use super::*;
	use crate::weights::WeightInfo;

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
		type LocalOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = DomainAddress,
		>;

		/// The AdminOrigin ensures that some calls can only be performed by
		/// admins.
		type AdminOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// The incoming and outgoing message type.
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
		type Connectors: InboundQueue<Sender = Domain, Message = Self::Message>;

		type WeightInfo: WeightInfo;

		/// Maximum number of submitter for a domain.
		#[pallet::constant]
		type MaxSubmittersPerDomain: Get<u32>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The router for a given domain was set.
		DomainRouterSet { domain: Domain, router: T::Router },

		/// A submitter was added to a domain.
		SubmitterAdded(DomainAddress),

		/// A submitter was removed from a domain.
		SubmitterRemoved(DomainAddress),
	}

	/// Storage for domain routers.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	pub(crate) type DomainRouters<T: Config> = StorageMap<_, Blake2_128Concat, Domain, T::Router>;

	/// Storage for domain submitters.
	///
	/// There is a limited number of submitters for each domain.
	///
	/// This can only be modified by an admin.
	#[pallet::storage]
	pub(crate) type DomainSubmitters<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		Domain,
		BoundedVec<DomainAddress, T::MaxSubmittersPerDomain>,
		ValueQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// The origin of the message to be processed is invalid.
		InvalidMessageOrigin,

		/// The domain is not supported.
		DomainNotSupported,

		/// Ethereum message decoding error.
		EthereumMessageDecode,

		/// Submitter was already added to the domain.
		SubmitterAlreadyAdded,

		/// Maximum number of submitters for a domain was reached.
		MaxSubmittersReached,

		/// Submitter was not found.
		SubmitterNotFound,

		/// Unknown submitter.
		UnknownSubmitter,

		/// Router not found.
		RouterNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set a domain's router,
		#[pallet::weight(< T as Config >::WeightInfo::set_domain_router())]
		#[pallet::call_index(0)]
		pub fn set_domain_router(
			origin: OriginFor<T>,
			domain: Domain,
			router: T::Router,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			ensure!(domain != Domain::Centrifuge, Error::<T>::DomainNotSupported);

			<DomainRouters<T>>::insert(domain.clone(), router.clone());

			Self::deposit_event(Event::DomainRouterSet { domain, router });

			Ok(())
		}

		/// Add a submitter for a specific domain.
		#[pallet::weight(< T as Config >::WeightInfo::add_submitter())]
		#[pallet::call_index(1)]
		pub fn add_submitter(origin: OriginFor<T>, submitter: DomainAddress) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			ensure!(
				submitter.domain() != Domain::Centrifuge,
				Error::<T>::DomainNotSupported
			);

			<DomainSubmitters<T>>::try_mutate(submitter.domain(), |submitters| {
				if submitters.iter().find(|s| s.eq(&&submitter)).is_some() {
					return Err(Error::<T>::SubmitterAlreadyAdded.into());
				}

				submitters
					.try_push(submitter.clone())
					.map_err(|_| Error::<T>::MaxSubmittersReached)?;

				Self::deposit_event(Event::SubmitterAdded(submitter));

				Ok(())
			})
		}

		/// Remove a submitter from a specific domain.
		#[pallet::weight(< T as Config >::WeightInfo::remove_submitter())]
		#[pallet::call_index(2)]
		pub fn remove_submitter(origin: OriginFor<T>, submitter: DomainAddress) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			<DomainSubmitters<T>>::try_mutate(submitter.domain(), |submitters| {
				let index = submitters
					.iter()
					.position(|s| s.eq(&submitter))
					.ok_or(Error::<T>::SubmitterNotFound)?;

				submitters.remove(index);

				Self::deposit_event(Event::SubmitterRemoved(submitter));

				Ok(())
			})
		}

		/// Process an incoming message.
		#[pallet::weight(0)]
		#[pallet::call_index(3)]
		pub fn process_msg(origin: OriginFor<T>, msg: Vec<u8>) -> DispatchResult {
			let domain_address = T::LocalOrigin::ensure_origin(origin)?;

			match domain_address {
				DomainAddress::EVM(_, _) => {
					DomainSubmitters::<T>::get(domain_address.domain())
						.iter()
						.find(|s| s.eq(&&domain_address))
						.ok_or(Error::<T>::UnknownSubmitter)?;

					let incoming_msg = T::Message::deserialize(&mut msg.as_slice())
						.map_err(|_| Error::<T>::EthereumMessageDecode)?;

					T::Connectors::handle(domain_address.domain(), incoming_msg)
				}
				DomainAddress::Centrifuge(_) => Err(Error::<T>::InvalidMessageOrigin.into()),
			}
		}
	}

	impl<T: Config> OutboundQueue for Pallet<T> {
		type Destination = Domain;
		type Message = T::Message;
		type Sender = T::AccountId;

		fn handle(
			destination: Self::Destination,
			sender: Self::Sender,
			msg: Self::Message,
		) -> DispatchResult {
			ensure!(
				destination != Domain::Centrifuge,
				"cannot send message to local domain"
			);

			let router = DomainRouters::<T>::get(destination).ok_or(Error::<T>::RouterNotFound)?;

			router.send(sender, msg)
		}
	}
}
