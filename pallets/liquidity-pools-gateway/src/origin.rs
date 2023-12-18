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

use cfg_types::domain_address::DomainAddress;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::EnsureOrigin;
use scale_info::TypeInfo;
#[cfg(feature = "runtime-benchmarks")]
use sp_core::H160;
use sp_runtime::RuntimeDebug;

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum GatewayOrigin {
	Domain(DomainAddress),
	AxelarRelay(DomainAddress),
}

pub struct EnsureLocal;

impl<O: Into<Result<GatewayOrigin, O>> + From<GatewayOrigin>> EnsureOrigin<O> for EnsureLocal {
	type Success = GatewayOrigin;

	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into()
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<O, ()> {
		Ok(O::from(GatewayOrigin::Domain(DomainAddress::EVM(
			1,
			H160::from_low_u64_be(1).into(),
		))))
	}
}
