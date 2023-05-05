// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
#![cfg_attr(not(feature = "std"), no_std)]
use sp_std::convert::TryInto;

pub mod weights;

mod origin;
pub use origin::*;

#[frame_support::pallet]
pub mod pallet {
	use core::fmt::Debug;

	use cfg_traits::connectors::InboundQueue;
	use cfg_types::domain_address::{Domain, DomainAddress, EVMChainId};
	use codec::EncodeLike;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	use super::*;
	use crate::weights::WeightInfo;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type WeightInfo: WeightInfo;

		/// The LocalOrigin ensures that some calls can only be performed from a
		/// local context i.e. a pallet.
		type LocalOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = DomainAddress>;

		/// The AdminOrigin ensures that some calls can only be performed by
		/// admins.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The incoming and outgoing message type.
		type Message: sp_std::default::Default; // TODO(cdamian): remove default

		/// The type that holds information related to a domain router.
		///
		/// NOTE - this is the Router found in pallet-connectors.
		type DomainRouter: Encode
			+ Decode
			+ Clone
			+ PartialEq
			+ Debug
			+ MaxEncodedLen
			+ EncodeLike
			+ TypeInfo;

		/// The type that processes incoming messages.
		///
		/// NOTE - The InboundQueue trait should be implemented by
		/// pallet-connectors.
		type Connectors: InboundQueue<Sender = Domain, Message = Self::Message>;

		/// Maximum size of an Ethereum message.
		#[pallet::constant]
		type MaxEthMsg: Get<u32>;

		/// Maximum number of submitter for a domain.
		#[pallet::constant]
		type MaxSubmittersPerDomain: Get<u32>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The Router for a given domain was set.
		DomainRouterSet {
			domain: Domain,
			router: T::DomainRouter,
		},

		/// A submitter was added.
		SubmitterAdded(DomainAddress),

		/// A submitter was removed.
		SubmitterRemoved(DomainAddress),
	}

	/// Storage for domain routers.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	pub(crate) type DomainRouter<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, T::DomainRouter>;

	/// Storage for domain submitters.
	///
	/// There is a limited number of submitters for each domain.
	///
	/// This can only be modified by an admin.
	#[pallet::storage]
	pub(crate) type Submitter<T: Config> = StorageMap<
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

		/// Ethereum message decoding error.
		EthereumMessageDecode,

		/// Maximum number of submitters for a domain was reached.
		MaxSubmittersReached,

		/// Submitter was not found.
		SubmitterNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set a Domain's router,
		#[pallet::weight(< T as Config >::WeightInfo::set_domain_router())]
		#[pallet::call_index(0)]
		pub fn set_domain_router(
			origin: OriginFor<T>,
			domain: Domain,
			router: T::DomainRouter,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			<DomainRouter<T>>::insert(domain.clone(), router.clone());

			Self::deposit_event(Event::DomainRouterSet { domain, router });

			Ok(())
		}

		#[pallet::weight(< T as Config >::WeightInfo::add_submitter())]
		#[pallet::call_index(1)]
		pub fn add_submitter(origin: OriginFor<T>, submitter: DomainAddress) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			<Submitter<T>>::try_mutate(submitter.domain(), |submitters| {
				submitters
					.try_push(submitter.clone())
					.map_err(|_| Error::<T>::MaxSubmittersReached)?;

				Self::deposit_event(Event::SubmitterAdded(submitter));

				Ok(())
			})
		}

		#[pallet::weight(< T as Config >::WeightInfo::remove_submitter())]
		#[pallet::call_index(2)]
		pub fn remove_submitter(origin: OriginFor<T>, submitter: DomainAddress) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			<Submitter<T>>::try_mutate(submitter.domain(), |submitters| {
				let index = submitters
					.iter()
					.position(|s| s.eq(&submitter))
					.ok_or(Error::<T>::SubmitterNotFound)?;

				submitters.remove(index);

				Self::deposit_event(Event::SubmitterRemoved(submitter));

				Ok(())
			})
		}

		#[pallet::weight(0)]
		#[pallet::call_index(3)]
		pub fn process_msg(
			origin: OriginFor<T>,
			msg: BoundedVec<u8, T::MaxEthMsg>,
		) -> DispatchResult {
			let domain_address = T::LocalOrigin::ensure_origin(origin)?;

			match domain_address {
				DomainAddress::EVM(chain_id, eth_address) => {
					let (sender, incoming_msg) =
						Self::decode_and_verify_ethereum_message(chain_id, msg)
							.map_err(|_| Error::<T>::EthereumMessageDecode)?;

					T::Connectors::submit(sender, incoming_msg)
				}
				DomainAddress::Centrifuge(_) => Err(Error::<T>::InvalidMessageOrigin.into()),
			}
		}
	}

	impl<T: Config> Pallet<T> {
		// TODO(cdamian): Implement
		fn decode_and_verify_ethereum_message(
			_chain_id: EVMChainId,
			_msg: BoundedVec<u8, T::MaxEthMsg>,
		) -> Result<(Domain, T::Message), ()> {
			Ok((Domain::Centrifuge, T::Message::default()))
		}
	}
}
