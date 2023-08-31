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
use sp_std::{convert::TryInto, vec::Vec};

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
	const BYTES_U32: usize = 4;

	use cfg_traits::TryConvert;

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

		/// A way to recover a domain address from two byte slices
		type OriginRecovery: TryConvert<(Vec<u8>, Vec<u8>), DomainAddress, Error = DispatchError>;

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

		/// A relayer was added.
		RelayerAdded { relayer: DomainAddress },

		/// A relayer was removed.
		RelayerRemoved { relayer: DomainAddress },
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

	/// Storage that contains a limited number of whitelisted instances of
	/// deployed liquidity pools for a particular domain.
	///
	/// This can only be modified by an admin.
	#[pallet::storage]
	#[pallet::getter(fn relayer)]
	pub(crate) type RelayerList<T: Config> =
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
		RelayerMessageDecodingFailed,
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

		/// Remove a instance from a specific domain.
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
		#[pallet::weight(0)]
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
						RelayerList::<T>::contains_key(
							domain_address.domain(),
							domain_address.clone()
						),
						Error::<T>::UnknownRelayer
					);

					// Every axelar relay will prepend the (sourceChain,
					// sourceAddress) from actual origination chain to the
					// message bytes, with a length identifier
					let slice_ref = &mut msg.as_slice();
					let length_source_chain: usize =
						Pallet::<T>::try_range(slice_ref, BYTES_U32, |be_bytes_u32| {
							let mut bytes = [0u8; BYTES_U32];
							// NOTE: This can NEVER panic as the `try_range` logic ensures the given
							// bytes have the right length. I.e. 4 in this case/
							bytes.copy_from_slice(&be_bytes_u32);

							Ok(u32::from_be_bytes(bytes).try_into().map_err(|_| {
								DispatchError::Other("Expect: usize in wasm is always ge u32")
							})?)
						})?;

					let source_chain =
						Pallet::<T>::try_range(slice_ref, length_source_chain, |source_chain| {
							Ok(source_chain.to_vec())
						})?;

					let length_source_address: usize =
						Pallet::<T>::try_range(slice_ref, BYTES_U32, |be_bytes_u32| {
							let mut bytes = [0u8; BYTES_U32];
							// NOTE: This can NEVER panic as the `try_range` logic ensures the given
							// bytes have the right length. I.e. 4 in this case/
							bytes.copy_from_slice(&be_bytes_u32);

							Ok(u32::from_be_bytes(bytes).try_into().map_err(|_| {
								DispatchError::Other("Expect: usize in wasm is always ge u32")
							})?)
						})?;

					let source_address =
						Pallet::<T>::try_range(slice_ref, length_source_address, |source_chain| {
							Ok(source_chain.to_vec())
						})?;

					let origin_msg = Pallet::<T>::try_range(slice_ref, slice_ref.len(), |msg| {
						Ok(BoundedVec::try_from(msg.to_vec()).map_err(|_| {
							DispatchError::Other(
								"Remaining bytes smaller vector in the first place. qed.",
							)
						})?)
					})?;

					let origin_domain =
						T::OriginRecovery::try_convert((source_chain, source_address))?;

					Pallet::<T>::validate(origin_domain, origin_msg)?
				}
			};

			T::InboundQueue::submit(domain_address, incoming_msg)
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn try_range<'a, D, F>(
			slice: &mut &'a [u8],
			next_steps: usize,
			transformer: F,
		) -> Result<D, DispatchError>
		where
			F: Fn(&'a [u8]) -> Result<D, DispatchError>,
		{
			ensure!(
				slice.len() >= next_steps,
				Error::<T>::RelayerMessageDecodingFailed
			);

			let (input, new_slice) = slice.split_at(next_steps);
			let res = transformer(&input)?;
			*slice = &mut &new_slice;

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

			let incoming_msg = T::Message::deserialize(&mut msg.as_slice())
				.map_err(|_| Error::<T>::MessageDecodingFailed)?;

			Ok((address, incoming_msg))
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
