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

use cfg_primitives::AccountId;
use pallet_evm::AddressMapping;
use polkadot_parachain::primitives::Sibling;
use sp_core::{crypto::AccountId32, Get, H160};
use sp_runtime::traits::AccountIdConversion;

use crate::account_conversion::AccountConverter;

pub struct GatewayAccountProvider<T, XcmConverter>(core::marker::PhantomData<(T, XcmConverter)>);

impl<T, XcmConverter> GatewayAccountProvider<T, XcmConverter>
where
	T: pallet_evm_chain_id::Config + parachain_info::Config,
	XcmConverter: xcm_executor::traits::Convert<xcm::v3::MultiLocation, AccountId>,
{
	pub fn get_gateway_account() -> AccountId {
		let sender_account: AccountId =
			Sibling::from(parachain_info::Pallet::<T>::get()).into_account_truncating();

		let truncated_sender_account =
			H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&sender_account)[0..20]);

		AccountConverter::<T, XcmConverter>::into_account_id(truncated_sender_account)
	}
}

// NOTE: Can be removed once all runtimes implement a true InboundQueue
pub mod stump_queue {
	use cfg_traits::liquidity_pools::InboundQueue;
	use cfg_types::domain_address::{Domain, DomainAddress};
	use sp_runtime::DispatchResult;
	use sp_std::marker::PhantomData;

	/// A stump inbound queue that does not yet hit the LP logic (before FI we
	/// do not want that) but stores an Event.
	pub struct StumpInboundQueue<Runtime, RuntimeEvent>(PhantomData<(Runtime, RuntimeEvent)>);
	impl<Runtime, RuntimeEvent> InboundQueue for StumpInboundQueue<Runtime, RuntimeEvent>
	where
		Runtime: pallet_liquidity_pools::Config + frame_system::Config,
	{
		type Message = pallet_liquidity_pools::Message<
			Domain,
			<Runtime as pallet_liquidity_pools::Config>::PoolId,
			<Runtime as pallet_liquidity_pools::Config>::TrancheId,
			<Runtime as pallet_liquidity_pools::Config>::Balance,
			<Runtime as pallet_liquidity_pools::Config>::BalanceRatio,
		>;
		type Sender = DomainAddress;

		fn submit(sender: Self::Sender, message: Self::Message) -> DispatchResult {
			let event = {
				let event =
					pallet_liquidity_pools::Event::<Runtime>::IncomingMessage { sender, message };

				// Mirror deposit_event logic here as it is private
				let event = <<Runtime as pallet_liquidity_pools::Config>::RuntimeEvent as From<
					pallet_liquidity_pools::Event<Runtime>,
				>>::from(event);

				<<Runtime as pallet_liquidity_pools::Config>::RuntimeEvent as Into<
					<Runtime as frame_system::Config>::RuntimeEvent,
				>>::into(event)
			};

			// Triggering only the event for error resolution
			frame_system::pallet::Pallet::<Runtime>::deposit_event(event);

			Ok(())
		}
	}
}
