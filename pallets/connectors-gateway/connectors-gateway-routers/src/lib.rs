// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

use cfg_traits::connectors::Router;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchResult;
use scale_info::TypeInfo;

pub mod moonbeam;
pub use crate::moonbeam::*;

type CurrencyIdOf<T> = <T as pallet_xcm_transactor::Config>::CurrencyId;
type MessageOf<T> = <T as pallet_connectors_gateway::Config>::Message;
type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum DomainRouter<T>
where
	T: frame_system::Config + pallet_xcm_transactor::Config + pallet_connectors_gateway::Config,
{
	EthereumXCM(EthereumXCMRouter<T>),
}

impl<T> Router for DomainRouter<T>
where
	T: frame_system::Config + pallet_xcm_transactor::Config + pallet_connectors_gateway::Config,
{
	type Message = MessageOf<T>;
	type Sender = AccountIdOf<T>;

	fn send(&self, sender: Self::Sender, message: Self::Message) -> DispatchResult {
		match self {
			DomainRouter::EthereumXCM(r) => r.do_send(sender, message),
		}
	}
}
