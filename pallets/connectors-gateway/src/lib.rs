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
extern crate alloc;

use sp_std::convert::TryInto;

mod origin;
pub use origin::*;

mod router;
pub use router::*;

pub mod weights;

pub type CurrencyIdOf<T> = <T as pallet_xcm_transactor::Config>::CurrencyId;

#[frame_support::pallet]
pub mod pallet {
	use core::convert::TryFrom;

	use cfg_traits::connectors::{Codec, InboundQueue, OutboundQueue};
	use cfg_types::domain_address::{Domain, DomainAddress};
	use ethabi::{Bytes, Contract};
	use frame_support::{pallet_prelude::*, traits::OriginTrait};
	use frame_system::pallet_prelude::OriginFor;
	use pallet_xcm_transactor::{Currency, CurrencyPayment, TransactWeights};
	use sp_core::U256;
	use sp_std::{vec, vec::Vec};
	use xcm::v0::OriginKind;

	use super::*;
	use crate::weights::WeightInfo;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_xcm_transactor::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type WeightInfo: WeightInfo;

		/// The LocalOrigin ensures that some calls can only be performed from a
		/// local context i.e. a different pallet.
		type LocalOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = DomainAddress>;

		/// The AdminOrigin ensures that some calls can only be performed by
		/// admins.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The incoming and outgoing message type.
		type Message: Codec;

		/// The type that processes incoming messages.
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
		/// The router for a given domain was set.
		DomainRouterSet {
			domain: Domain,
			router: Router<CurrencyIdOf<T>>,
		},

		/// A submitter was added to a domain.
		SubmitterAdded(DomainAddress),

		/// A submitter was removed from a domain.
		SubmitterRemoved(DomainAddress),
	}

	// pub enum Router {
	//  Xcmv1(v1::XcmRouter)
	//  Xcmv2(v2::XcmRouter)
	// }

	/// Storage for domain routers.
	///
	/// This can only be set by an admin.
	#[pallet::storage]
	pub(crate) type DomainRouters<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, Router<CurrencyIdOf<T>>>;

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

		/// Ethereum message decoding error.
		EthereumMessageDecode,

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
		/// Set a Domain's router,
		#[pallet::weight(< T as Config >::WeightInfo::set_domain_router())]
		#[pallet::call_index(0)]
		pub fn set_domain_router(
			origin: OriginFor<T>,
			domain: Domain,
			router: Router<CurrencyIdOf<T>>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			<DomainRouters<T>>::insert(domain.clone(), router.clone());

			Self::deposit_event(Event::DomainRouterSet { domain, router });

			Ok(())
		}

		#[pallet::weight(< T as Config >::WeightInfo::add_submitter())]
		#[pallet::call_index(1)]
		pub fn add_submitter(origin: OriginFor<T>, submitter: DomainAddress) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			<DomainSubmitters<T>>::try_mutate(submitter.domain(), |submitters| {
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

		#[pallet::weight(0)]
		#[pallet::call_index(3)]
		pub fn process_msg(
			origin: OriginFor<T>,
			msg: BoundedVec<u8, T::MaxEthMsg>,
		) -> DispatchResult {
			let domain_address = T::LocalOrigin::ensure_origin(origin)?;

			match domain_address {
				DomainAddress::EVM(_, _) => {
					DomainSubmitters::<T>::get(domain_address.domain())
						.iter()
						.find(|s| s.eq(&&domain_address))
						.ok_or(Error::<T>::UnknownSubmitter)?;

					let incoming_msg = T::Message::deserialize(&mut msg.as_slice())
						.map_err(|_| Error::<T>::EthereumMessageDecode)?;

					T::Connectors::submit(domain_address.domain(), incoming_msg)
				}
				DomainAddress::Centrifuge(_) => Err(Error::<T>::InvalidMessageOrigin.into()),
			}
		}
	}

	impl<T: Config> Pallet<T> {
		/// COPIED FROM CONNECTORS
		///
		/// Build the encoded `ethereum_xcm::transact(eth_tx)` call that should
		/// request to execute `evm_call`.
		///
		/// * `xcm_domain` - All the necessary info regarding the xcm-based
		///   domain
		/// where this `ethereum_xcm` call is to be executed
		/// * `evm_call` - The encoded EVM call calling
		///   ConnectorsXcmRouter::handle(msg)
		pub fn encoded_ethereum_xcm_call(
			xcm_domain: XcmDomain<T::CurrencyId>,
			evm_call: Vec<u8>,
		) -> Vec<u8> {
			let mut encoded: Vec<u8> = Vec::new();

			encoded.append(
				&mut xcm_domain
					.ethereum_xcm_transact_call_index
					.clone()
					.into_inner(),
			);
			encoded.append(
				&mut xcm_primitives::EthereumXcmTransaction::V1(
					xcm_primitives::EthereumXcmTransactionV1 {
						gas_limit: U256::from(xcm_domain.max_gas_limit),
						fee_payment: xcm_primitives::EthereumXcmFee::Auto,
						action: pallet_ethereum::TransactionAction::Call(
							xcm_domain.contract_address,
						),
						value: U256::zero(),
						input: BoundedVec::<
							u8,
							ConstU32<{ xcm_primitives::MAX_ETHEREUM_XCM_INPUT_SIZE }>,
						>::try_from(evm_call)
						.unwrap(),
						access_list: None,
					},
				)
				.encode(),
			);

			encoded
		}
	}

	impl<T: Config> OutboundQueue for Pallet<T> {
		type Destination = Domain;
		type Message = T::Message;
		type Sender = T::AccountId;

		fn submit(
			destination: Self::Destination,
			sender: Self::Sender,
			msg: Self::Message,
		) -> DispatchResult {
			let Router::Xcm(xcm_domain) =
				DomainRouters::<T>::get(destination).ok_or(Error::<T>::RouterNotFound)?;

			let contract_call = encoded_contract_call(msg.serialize());
			let ethereum_xcm_call =
				Pallet::<T>::encoded_ethereum_xcm_call(xcm_domain.clone(), contract_call);

			pallet_xcm_transactor::Pallet::<T>::transact_through_sovereign(
				T::RuntimeOrigin::root(),
				// The destination to which the message should be sent
				xcm_domain.location,
				// The sender will pay for this transaction.
				sender,
				// The currency in which we want to pay fees
				CurrencyPayment {
					currency: Currency::AsCurrencyId(xcm_domain.fee_currency),
					fee_amount: None,
				},
				// The call to be executed in the destination chain
				ethereum_xcm_call,
				OriginKind::SovereignAccount,
				TransactWeights {
					// Convert the max gas_limit into a max transact weight following Moonbeam's
					// formula.
					transact_required_weight_at_most: xcm_domain.max_gas_limit * 25_000
						+ 100_000_000,
					overall_weight: None,
				},
			)?;

			Ok(())
		}
	}

	/// COPIED FROM CONNECTORS
	///
	/// The ConnectorsXcmContract handle function name
	static HANDLE_FUNCTION: &str = "handle";

	/// Return the encoded contract call, i.e,
	/// ConnectorsXcmRouter::handle(encoded_msg).
	pub fn encoded_contract_call(encoded_msg: Vec<u8>) -> Bytes {
		let contract = xcm_router_contract();
		let encoded_contract_call = contract
			.function(HANDLE_FUNCTION)
			.expect("Known at compilation time")
			.encode_input(&[ethabi::Token::Bytes(encoded_msg)])
			.expect("Known at compilation time");

		encoded_contract_call
	}

	/// COPIED FROM CONNECTORS
	///
	/// The ConnectorsXcmRouter Abi as in ethabi::Contract
	/// Note: We only concern ourselves with the `handle` function of the
	/// contract since that's all we need to build the calls for remote EVM
	/// execution.
	pub fn xcm_router_contract() -> Contract {
		use sp_std::collections::btree_map::BTreeMap;

		let mut functions = BTreeMap::new();
		#[allow(deprecated)]
		functions.insert(
			"handle".into(),
			vec![ethabi::Function {
				name: HANDLE_FUNCTION.into(),
				inputs: vec![ethabi::Param {
					name: "message".into(),
					kind: ethabi::ParamType::Bytes,
					internal_type: None,
				}],
				outputs: vec![],
				constant: false,
				state_mutability: Default::default(),
			}],
		);

		ethabi::Contract {
			constructor: None,
			functions,
			events: Default::default(),
			errors: Default::default(),
			receive: false,
			fallback: false,
		}
	}
}
