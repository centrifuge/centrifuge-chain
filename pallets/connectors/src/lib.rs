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
use core::convert::TryFrom;

use cfg_traits::{connectors::Codec, ForeignInvestments, PoolInspect};
use cfg_types::{
	domain_address::{Domain, DomainAddress, DomainLocator},
	tokens::GeneralCurrencyIndex,
};
use cfg_utils::vec_to_fixed_array;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::{
	fungibles::{Inspect, Mutate, Transfer},
	OriginTrait,
};
use orml_traits::asset_registry::{self, Inspect as _};
pub use pallet::*;
use scale_info::TypeInfo;
use sp_core::U256;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Convert},
	FixedPointNumber,
};
use sp_std::{convert::TryInto, vec, vec::Vec};
use xcm::VersionedMultiLocation;
pub mod weights;

mod message;
pub use message::*;

mod routers;
pub use routers::*;

mod contract;
pub use contract::*;

mod inbound;

/// The Parachains that Centrifuge Connectors support.
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum ParachainId {
	/// Moonbeam - It may be Moonbeam on Polkadot, Moonriver on Kusama, or
	/// Moonbase on a testnet.
	Moonbeam,
}

// Type aliases
pub type PoolIdOf<T> = <<T as Config>::PoolInspect as PoolInspect<
	<T as frame_system::Config>::AccountId,
	CurrencyIdOf<T>,
>>::PoolId;

pub type TrancheIdOf<T> = <<T as Config>::PoolInspect as PoolInspect<
	<T as frame_system::Config>::AccountId,
	CurrencyIdOf<T>,
>>::TrancheId;

pub type MessageOf<T> =
	Message<Domain, PoolIdOf<T>, TrancheIdOf<T>, <T as Config>::Balance, <T as Config>::Rate>;

pub type CurrencyIdOf<T> = <T as Config>::CurrencyId;

pub type GeneralCurrencyIndexType = u128;

pub type GeneralCurrencyIndexOf<T> =
	GeneralCurrencyIndex<GeneralCurrencyIndexType, <T as pallet::Config>::GeneralCurrencyPrefix>;

#[frame_support::pallet]
pub mod pallet {
	use cfg_primitives::Moment;
	use cfg_traits::{
		CurrencyInspect, Investment, InvestmentAccountant, InvestmentCollector,
		InvestmentProperties, Permissions, PoolInspect, TrancheCurrency,
	};
	use cfg_types::{
		permissions::{PermissionScope, PoolRole, Role},
		tokens::{ConnectorsWrappedCurrency, CustomMetadata},
	};
	use frame_support::{pallet_prelude::*, traits::UnixTime};
	use frame_system::pallet_prelude::*;
	use pallet_xcm_transactor::{Currency, CurrencyPayment, TransactWeights};
	use sp_runtime::traits::{AccountIdConversion, Zero};
	use xcm::{latest::OriginKind, v3::MultiLocation};

	use super::*;
	use crate::weights::WeightInfo;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_xcm_transactor::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type WeightInfo: WeightInfo;

		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		type Rate: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber + TypeInfo;

		/// The origin allowed to make admin-like changes, such calling
		/// `set_domain_router`.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type PoolInspect: PoolInspect<Self::AccountId, CurrencyIdOf<Self>, Rate = Self::Rate>;

		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<PoolIdOf<Self>, CurrencyIdOf<Self>>,
			Role = Role<TrancheIdOf<Self>, Moment>,
			Error = DispatchError,
		>;

		type Time: UnixTime;

		type Tokens: Mutate<Self::AccountId>
			+ Inspect<
				Self::AccountId,
				AssetId = CurrencyIdOf<Self>,
				Balance = <Self as pallet::Config>::Balance,
			> + Transfer<Self::AccountId>;

		type TrancheCurrency: TrancheCurrency<PoolIdOf<Self>, TrancheIdOf<Self>>
			+ Into<CurrencyIdOf<Self>>
			+ Clone;

		type ForeignInvestment: Investment<
				Self::AccountId,
				Error = DispatchError,
				InvestmentId = <Self as Config>::TrancheCurrency,
				Amount = <Self as Config>::Balance,
			> + InvestmentCollector<
				Self::AccountId,
				Error = DispatchError,
				InvestmentId = <Self as Config>::TrancheCurrency,
				Result = (),
			>;

		type ForeignInvestmentAccountant: InvestmentAccountant<
			Self::AccountId,
			Amount = <Self as Config>::Balance,
			Error = DispatchError,
			InvestmentId = <Self as Config>::TrancheCurrency,
		>;

		type AssetRegistry: asset_registry::Inspect<
			AssetId = CurrencyIdOf<Self>,
			Balance = <Self as Config>::Balance,
			CustomMetadata = CustomMetadata,
		>;

		/// The currency type of transferrable token.
		type CurrencyId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen
			+ Into<<Self as pallet_xcm_transactor::Config>::CurrencyId>
			+ TryInto<GeneralCurrencyIndexOf<Self>, Error = DispatchError>
			+ TryFrom<GeneralCurrencyIndexOf<Self>, Error = DispatchError>
			+ From<
				<<<Self as Config>::ForeignInvestmentAccountant as InvestmentAccountant<
					Self::AccountId,
				>>::InvestmentInfo as InvestmentProperties<Self::AccountId>>::Currency,
			> + CurrencyInspect<CurrencyId = <Self as pallet::Config>::CurrencyId>;

		/// The converter from a DomainAddress to a Substrate AccountId
		type AccountConverter: Convert<DomainAddress, Self::AccountId>;

		/// The converter from a [ConnectorsWrappedCurrency] to `MultiLocation`.
		type CurrencyConverter: Convert<ConnectorsWrappedCurrency, MultiLocation>
			+ Convert<MultiLocation, Result<ConnectorsWrappedCurrency, ()>>
			+ Convert<VersionedMultiLocation, Result<ConnectorsWrappedCurrency, ()>>;

		/// The prefix for currencies added via Connectors.
		#[pallet::constant]
		type GeneralCurrencyPrefix: Get<[u8; 12]>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A message was sent to a domain
		MessageSent {
			message: MessageOf<T>,
			domain: Domain,
		},

		/// The Router for a given domain was set
		SetDomainRouter {
			domain: Domain,
			router: Router<CurrencyIdOf<T>>,
		},

		IncomingMessage {
			sender: T::AccountId,
			message: Vec<u8>,
		},
	}

	#[pallet::storage]
	pub(crate) type DomainRouter<T: Config> =
		StorageMap<_, Blake2_128Concat, Domain, Router<CurrencyIdOf<T>>>;

	/// The set of known connectors. This set is used as an allow-list when
	/// authorizing the origin of incoming messages through the `handle`
	/// extrinsic.
	#[pallet::storage]
	pub(crate) type KnownConnectors<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ()>;

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to map the asset to the corresponding Connector's General
		/// Index representation and thus cannot be used as an
		/// investment currency.
		AssetNotFound,
		/// The metadata of the given asset does not declare it as a pool
		/// currency and thus it cannot be used as an investment currency.
		AssetMetadataNotPoolCurrency,
		/// The metadata of the given asset does not declare it as transferable
		/// via connectors.
		AssetNotConnectorsTransferable,
		/// The asset is not a [ConnectorsWrappedCurrency] and thus cannot be
		/// transferred via connectors.
		AssetNotConnectorsWrappedCurrency,
		/// The given asset does not match the currency of the pool.
		AssetNotPoolCurrency,
		/// A pool could not be found.
		PoolNotFound,
		/// A tranche could not be found.
		TrancheNotFound,
		/// Could not find the metadata of a tranche token.
		TrancheMetadataNotFound,
		/// Failed to fetch a tranche token price.
		/// This can occur if `TrancheNotFound` or if effectively
		/// the price for this tranche has not yet been set.
		MissingTranchePrice,
		/// Router not set for a given domain.
		MissingRouter,
		/// Transfer amount must be non-zero.
		InvalidTransferAmount,
		/// A transfer to a non-whitelisted destination was attempted.
		UnauthorizedTransfer,
		/// Failed to build Ethereum_Xcm call.
		FailedToBuildEthereumXcmCall,
		/// The origin of an incoming message is not in the allow-list.
		InvalidIncomingMessageOrigin,
		/// Failed to decode an incoming message.
		InvalidIncomingMessage,
		/// The destination domain is invalid.
		InvalidDomain,
		/// The validity is in the past.
		InvalidTrancheInvestorValidity,
		/// Failed to match the provided GeneralCurrencyIndex against the
		/// investment currency of the pool.
		InvalidInvestCurrency,
		/// The currency is not allowed to be transferred via Connectors.
		InvalidTransferCurrency,
		/// The domain has not been whitelisted as a TrancheInvestor.
		DomainNotWhitelisted,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as frame_system::Config>::AccountId: From<[u8; 32]>,
	{
		/// Set a Domain's router
		#[pallet::weight(< T as Config >::WeightInfo::set_domain_router())]
		#[pallet::call_index(0)]
		pub fn set_domain_router(
			origin: OriginFor<T>,
			domain: Domain,
			router: Router<CurrencyIdOf<T>>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin.clone())?;

			<DomainRouter<T>>::insert(domain.clone(), router.clone());
			Self::deposit_event(Event::SetDomainRouter { domain, router });

			Ok(())
		}

		/// Add an AccountId to the set of known connectors, allowing that
		/// origin to send incoming messages.
		#[pallet::weight(< T as Config >::WeightInfo::add_connector())]
		#[pallet::call_index(1)]
		pub fn add_connector(origin: OriginFor<T>, connector: T::AccountId) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			<KnownConnectors<T>>::insert(connector, ());

			Ok(())
		}

		/// Add a pool to a given domain
		#[pallet::weight(< T as Config >::WeightInfo::add_pool())]
		#[pallet::call_index(2)]
		pub fn add_pool(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			domain: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);

			Self::do_send_message(who, Message::AddPool { pool_id }, domain)?;

			Ok(())
		}

		/// Add a tranche to a given domain
		#[pallet::weight(< T as Config >::WeightInfo::add_tranche())]
		#[pallet::call_index(3)]
		pub fn add_tranche(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			decimals: u8,
			domain: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				T::PoolInspect::tranche_exists(pool_id, tranche_id),
				Error::<T>::TrancheNotFound
			);

			// Look up the metadata of the tranche token
			let currency_id = Self::derive_invest_id(pool_id, tranche_id)?;
			let metadata = T::AssetRegistry::metadata(&currency_id.into())
				.ok_or(Error::<T>::TrancheMetadataNotFound)?;
			let token_name = vec_to_fixed_array(metadata.name);
			let token_symbol = vec_to_fixed_array(metadata.symbol);
			let price = T::PoolInspect::get_tranche_token_price(pool_id, tranche_id)
				.ok_or(Error::<T>::MissingTranchePrice)?
				.price;

			// Send the message to the domain
			Self::do_send_message(
				who,
				Message::AddTranche {
					pool_id,
					tranche_id,
					decimals,
					token_name,
					token_symbol,
					price,
				},
				domain,
			)?;

			Ok(())
		}

		/// Update the price of a tranche token
		#[pallet::weight(< T as Config >::WeightInfo::update_token_price())]
		#[pallet::call_index(4)]
		pub fn update_token_price(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			domain: Domain,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// TODO(follow-up PR): Move `get_tranche_token_price` to new trait.
			// https://centrifuge.hackmd.io/SERpps-URlG4hkOyyS94-w?both#fn-update_tranche_token_price
			let price = T::PoolInspect::get_tranche_token_price(pool_id, tranche_id)
				.ok_or(Error::<T>::MissingTranchePrice)?
				.price;

			Self::do_send_message(
				who,
				Message::UpdateTrancheTokenPrice {
					pool_id,
					tranche_id,
					price,
				},
				domain,
			)?;

			Ok(())
		}

		/// Update a member
		#[pallet::weight(< T as Config >::WeightInfo::update_member())]
		#[pallet::call_index(5)]
		pub fn update_member(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			domain_address: DomainAddress,
			valid_until: Moment,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			ensure!(
				T::PoolInspect::tranche_exists(pool_id, tranche_id),
				Error::<T>::TrancheNotFound
			);
			ensure!(
				valid_until > Self::now(),
				Error::<T>::InvalidTrancheInvestorValidity
			);

			// Ensure that the destination address has been whitelisted as a TrancheInvestor
			// beforehand.
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					T::AccountConverter::convert(domain_address.clone()),
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, valid_until))
				),
				Error::<T>::DomainNotWhitelisted
			);

			Self::do_send_message(
				who,
				Message::UpdateMember {
					pool_id,
					tranche_id,
					valid_until,
					member: domain_address.address(),
				},
				domain_address.domain(),
			)?;

			Ok(())
		}

		/// Transfer tranche tokens to a given address
		#[pallet::weight(< T as Config >::WeightInfo::transfer())]
		#[pallet::call_index(6)]
		pub fn transfer_tranche_tokens(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
			domain_address: DomainAddress,
			amount: <T as pallet::Config>::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// Check that the destination is not the local domain
			ensure!(
				domain_address.domain() != Domain::Centrifuge,
				Error::<T>::InvalidDomain
			);
			ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					T::AccountConverter::convert(domain_address.clone()),
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, Self::now()))
				),
				Error::<T>::UnauthorizedTransfer
			);

			// Ensure pool and tranche exist and derive invest id
			let invest_id = Self::derive_invest_id(pool_id, tranche_id)?;
			ensure!(
				CurrencyIdOf::<T>::is_tranche_token(invest_id.clone().into()),
				Error::<T>::InvalidTransferCurrency
			);

			// Transfer to the domain account for bookkeeping
			T::Tokens::transfer(
				invest_id.into(),
				&who,
				&DomainLocator::<Domain> {
					domain: domain_address.domain(),
				}
				.into_account_truncating(),
				amount,
				false,
			)?;

			Self::do_send_message(
				who.clone(),
				Message::TransferTrancheTokens {
					pool_id,
					tranche_id,
					amount,
					domain: domain_address.domain(),
					sender: who
						.encode()
						.try_into()
						.map_err(|_| DispatchError::Other("Conversion to 32 bytes failed"))?,
					receiver: domain_address.address(),
				},
				domain_address.domain(),
			)?;

			Ok(())
		}

		/// Transfer non-tranche tokens to a given address
		#[pallet::weight(< T as Config >::WeightInfo::transfer())]
		#[pallet::call_index(7)]
		pub fn transfer(
			origin: OriginFor<T>,
			currency_id: CurrencyIdOf<T>,
			receiver: DomainAddress,
			amount: <T as pallet::Config>::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);
			// Check that the destination is not the local domain
			ensure!(
				receiver.domain() != Domain::Centrifuge,
				Error::<T>::InvalidDomain
			);
			ensure!(
				!CurrencyIdOf::<T>::is_tranche_token(currency_id),
				Error::<T>::InvalidTransferCurrency
			);
			let currency = Self::try_get_general_index(currency_id)?;

			// Check that the registered asset location matches the destination
			match Self::try_get_wrapped_currency(&currency_id)? {
				ConnectorsWrappedCurrency::EVM { chain_id, .. } => {
					ensure!(
						Domain::EVM(chain_id) == receiver.domain(),
						Error::<T>::InvalidDomain
					);
				}
			}

			// Transfer to the domain account for bookkeeping
			T::Tokens::transfer(
				currency_id,
				&who,
				&DomainLocator::<Domain> {
					domain: receiver.domain(),
				}
				.into_account_truncating(),
				amount,
				false,
			)?;

			Self::do_send_message(
				who.clone(),
				Message::Transfer {
					amount,
					currency,
					sender: who
						.encode()
						.try_into()
						.map_err(|_| DispatchError::Other("Conversion to 32 bytes failed"))?,
					receiver: receiver.address(),
				},
				receiver.domain(),
			)?;

			Ok(())
		}

		/// Add a currency to the set of known currencies on the domain derived
		/// from the given currency.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
		#[pallet::call_index(8)]
		pub fn add_currency(origin: OriginFor<T>, currency_id: CurrencyIdOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let currency = Self::try_get_general_index(currency_id)?;

			let ConnectorsWrappedCurrency::EVM {
				chain_id,
				address: evm_address,
			} = Self::try_get_wrapped_currency(&currency_id)?;

			Self::do_send_message(
				who,
				Message::AddCurrency {
					currency,
					evm_address,
				},
				Domain::EVM(chain_id),
			)?;

			Ok(())
		}

		/// Allow a currency to be used as a pool currency and to invest in a
		/// pool on the domain derived from the given currency.
		#[pallet::call_index(90)]
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
		pub fn allow_pool_currency(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			currency_id: CurrencyIdOf<T>,
		) -> DispatchResult {
			// TODO(subsequent PR): In the future, should be permissioned by trait which
			// does not exist yet.
			// See spec: https://centrifuge.hackmd.io/SERpps-URlG4hkOyyS94-w?view#fn-add_pool_currency

			// TODO(@review): According to spec, this should be restricted to
			// `AdminOrigin`. However, `do_send_message` requires a 32-byte address for the
			// payment of the fee. We could set the treasury.
			// https://centrifuge.hackmd.io/SERpps-URlG4hkOyyS94-w?view#fn-add_pool_currency
			// let who = T::AdminOrigin::ensure_origin(origin)?;
			let who = ensure_signed(origin)?;

			// Ensure currency matches the currency of the pool
			Self::can_invest_currency_into_pool(pool_id, currency_id)?;

			// Derive GeneralIndex for currency
			let currency = Self::try_get_general_index(currency_id)?;

			let ConnectorsWrappedCurrency::EVM { chain_id, .. } =
				Self::try_get_wrapped_currency(&currency_id)?;

			Self::do_send_message(
				who,
				Message::AllowPoolCurrency { pool_id, currency },
				Domain::EVM(chain_id),
			)?;

			Ok(())
		}

		/// Handle an incoming message
		/// TODO(nuno): we probably need a custom origin type for these messages
		/// to ensure they have come in through XCM. For now, let's have a POC
		/// here to test the pipeline Ethereum ---> Moonbeam --->
		/// Centrifuge::connectors
		/// TODO(subsequent PR): Should be removed as extrinsic before adding to
		/// prod runtimes. Should be handled automatically via Gateway.
		#[pallet::call_index(99)]
		#[pallet::weight(< T as Config >::WeightInfo::handle())]
		pub fn handle(origin: OriginFor<T>, bytes: Vec<u8>) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;
			ensure!(
				<KnownConnectors<T>>::contains_key(&sender),
				Error::<T>::InvalidIncomingMessageOrigin
			);

			Self::deposit_event(Event::IncomingMessage {
				sender,
				message: bytes.clone(),
			});
			let msg: MessageOf<T> = Message::deserialize(&mut bytes.as_slice())
				.map_err(|_| Error::<T>::InvalidIncomingMessage)?;

			// FIXME(subsequent PR): Derive domain via Gateway InboundQueue
			// blocked by https://github.com/centrifuge/centrifuge-chain/pull/1376
			let sending_domain: DomainAddress = DomainAddress::EVM(1284, [0u8; 20]);

			match msg {
				Message::Transfer {
					currency,
					receiver,
					amount,
					..
				} => Self::do_transfer_from_other_domain(currency.into(), receiver.into(), amount),
				Message::TransferTrancheTokens {
					pool_id,
					tranche_id,
					receiver,
					amount,
					..
				} => Self::do_transfer_tranche_tokens_from_other_domain(
					pool_id,
					tranche_id,
					sending_domain,
					receiver.into(),
					amount,
				),
				Message::IncreaseInvestOrder {
					pool_id,
					tranche_id,
					investor,
					currency,
					amount,
				} => Self::do_increase_invest_order(
					pool_id,
					tranche_id,
					investor.into(),
					currency.into(),
					amount,
				),
				Message::DecreaseInvestOrder {
					pool_id,
					tranche_id,
					investor,
					currency,
					amount,
				} => Self::do_decrease_invest_order(
					pool_id,
					tranche_id,
					investor.into(),
					currency.into(),
					amount,
				),
				Message::IncreaseRedeemOrder {
					pool_id,
					tranche_id,
					investor,
					amount,
					..
				} => Self::do_increase_redemption(
					pool_id,
					tranche_id,
					investor.into(),
					amount,
					sending_domain,
				),
				Message::DecreaseRedeemOrder {
					pool_id,
					tranche_id,
					investor,
					currency,
					amount,
				} => Self::do_decrease_redemption(
					pool_id,
					tranche_id,
					investor.into(),
					currency.into(),
					amount,
					sending_domain,
				),
				Message::CollectInvest {
					pool_id,
					tranche_id,
					investor,
				} => Self::do_collect_investment(pool_id, tranche_id, investor.into()),
				Message::CollectRedeem {
					pool_id,
					tranche_id,
					investor,
				} => Self::do_collect_redemption(pool_id, tranche_id, investor.into()),
				_ => Err(Error::<T>::InvalidIncomingMessage.into()),
			}?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn now() -> Moment {
			T::Time::now().as_secs()
		}

		/// Send the `message` to the given domain.
		pub fn do_send_message(
			fee_payer: T::AccountId,
			message: MessageOf<T>,
			domain: Domain,
		) -> DispatchResult {
			let Router::Xcm(xcm_domain) =
				<DomainRouter<T>>::get(domain.clone()).ok_or(Error::<T>::MissingRouter)?;

			#[cfg(feature = "std")]
			println!("Router: {:?}", xcm_domain);
			#[cfg(feature = "std")]
			println!("fee payer: {:?}", fee_payer);

			let contract_call = contract::encoded_contract_call(message.serialize());
			let ethereum_xcm_call =
				Self::encoded_ethereum_xcm_call(xcm_domain.clone(), contract_call);

			pallet_xcm_transactor::Pallet::<T>::transact_through_sovereign(
				T::RuntimeOrigin::root(),
				// The destination to which the message should be sent
				xcm_domain.location,
				fee_payer,
				// The currency in which we want to pay fees
				CurrencyPayment {
					currency: Currency::AsCurrencyId(xcm_domain.fee_currency.into()),
					fee_amount: None,
				},
				// The call to be executed in the destination chain
				ethereum_xcm_call,
				OriginKind::SovereignAccount,
				TransactWeights {
					// Convert the max gas_limit into a max transact weight following Moonbeam's
					// formula.
					transact_required_weight_at_most: Weight::from_ref_time(
						xcm_domain.max_gas_limit * 25_000 + 100_000_000,
					),
					overall_weight: None,
				},
			)?;

			Self::deposit_event(Event::MessageSent { message, domain });

			Ok(())
		}

		/// Build the encoded `ethereum_xcm::transact(eth_tx)` call that should
		/// request to execute `evm_call`.
		///
		/// * `xcm_domain` - All the necessary info regarding the xcm-based
		///   domain
		/// where this `ethereum_xcm` call is to be executed
		/// * `evm_call` - The encoded EVM call calling
		///   ConnectorsXcmRouter::handle(msg)
		pub fn encoded_ethereum_xcm_call(
			xcm_domain: XcmDomain<CurrencyIdOf<T>>,
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

		/// Returns the `u128` general index of a currency as the concatenation
		/// of the configured `GeneralCurrencyPrefix` and its local currency
		/// identifier.
		///
		/// Assumes the currency to be registered in the `AssetRegistry`.
		pub fn try_get_general_index(currency: CurrencyIdOf<T>) -> Result<u128, DispatchError> {
			ensure!(
				T::AssetRegistry::metadata(&currency).is_some(),
				Error::<T>::AssetNotFound
			);

			let general_index: GeneralCurrencyIndexOf<T> = CurrencyIdOf::<T>::try_into(currency)?;

			Ok(general_index.index)
		}

		/// Returns the local currency identifier from from its general index.
		///
		/// Assumes the currency to be registered in the `AssetRegistry`.
		///
		/// NOTE: Reverse operation of [try_get_general_index].
		pub fn try_get_currency_id(
			index: GeneralCurrencyIndexOf<T>,
		) -> Result<CurrencyIdOf<T>, DispatchError> {
			let currency = CurrencyIdOf::<T>::try_from(index)?;
			ensure!(
				T::AssetRegistry::metadata(&currency).is_some(),
				Error::<T>::AssetNotFound
			);

			Ok(currency)
		}

		pub fn try_get_wrapped_currency(
			currency_id: &CurrencyIdOf<T>,
		) -> Result<ConnectorsWrappedCurrency, DispatchError> {
			let meta = T::AssetRegistry::metadata(&currency_id).ok_or(Error::<T>::AssetNotFound)?;
			ensure!(
				meta.additional.transferability.includes_connectors(),
				Error::<T>::AssetNotConnectorsTransferable
			);
			T::CurrencyConverter::convert(meta.location.ok_or(Error::<T>::InvalidTransferCurrency)?)
				.map_err(|_| Error::<T>::AssetNotConnectorsWrappedCurrency.into())
		}

		/// Ensures that the given pool and tranche exists and returns the
		/// corresponding investment id.
		pub fn derive_invest_id(
			pool_id: PoolIdOf<T>,
			tranche_id: TrancheIdOf<T>,
		) -> Result<<T as pallet::Config>::TrancheCurrency, DispatchError> {
			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			ensure!(
				T::PoolInspect::tranche_exists(pool_id, tranche_id),
				Error::<T>::TrancheNotFound
			);

			Ok(TrancheCurrency::generate(pool_id, tranche_id))
		}

		/// Ensures that the payment currency of the given investment id matches
		/// the derived currency and returns the latter.
		pub fn try_get_payment_currency(
			invest_id: <T as pallet::Config>::TrancheCurrency,
			currency_index: GeneralCurrencyIndexOf<T>,
		) -> Result<CurrencyIdOf<T>, DispatchError> {
			// retrieve currency id from general index
			let currency = Self::try_get_currency_id(currency_index)?;

			// get investment info
			let payment_currency: CurrencyIdOf<T> =
				<T as pallet::Config>::ForeignInvestmentAccountant::info(invest_id)?
					.payment_currency()
					.into();
			ensure!(
				payment_currency == currency,
				Error::<T>::InvalidInvestCurrency
			);

			Ok(currency)
		}
	}

	impl<T: Config> cfg_traits::ForeignInvestments<CurrencyIdOf<T>> for Pallet<T> {
		type AssetRegistry = T::AssetRegistry;
		type Error = DispatchError;
		type PoolId = PoolIdOf<T>;

		fn can_invest_currency_into_pool(
			pool_id: PoolIdOf<T>,
			currency: CurrencyIdOf<T>,
		) -> DispatchResult {
			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);

			// Ensure metadata pool currency flag is enabled
			let metadata =
				T::AssetRegistry::metadata(&currency).ok_or(Error::<T>::AssetNotFound)?;
			ensure!(
				metadata.additional.pool_currency,
				Error::<T>::AssetMetadataNotPoolCurrency
			);

			T::PoolInspect::currency_for(pool_id)
				.filter(|c| c == &currency)
				.map(|_| ())
				.ok_or(Error::<T>::AssetNotPoolCurrency.into())
		}
	}
}

#[cfg(test)]
mod tests {
	use codec::{Decode, Encode};

	use crate::Domain;

	#[test]
	fn test_domain_encode_decode() {
		test_domain_identity(Domain::Centrifuge);
		test_domain_identity(Domain::EVM(1284));
		test_domain_identity(Domain::EVM(1));
	}

	/// Test that decode . encode results in the original value
	fn test_domain_identity(domain: Domain) {
		let encoded = domain.encode();
		let decoded: Domain = Domain::decode(&mut encoded.as_slice()).expect("");

		assert_eq!(domain, decoded);
	}
}
