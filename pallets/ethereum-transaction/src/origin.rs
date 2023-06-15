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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::EnsureOrigin;
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::RuntimeDebug;

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum EthereumOrigin {
	EthereumTransaction(H160),
}

pub struct EnsureEthereum;

impl<O: Into<Result<EthereumOrigin, O>> + From<EthereumOrigin>> EnsureOrigin<O> for EnsureEthereum {
	type Success = H160;

	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().map(|o| match o {
			EthereumOrigin::EthereumTransaction(id) => id,
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> O {
		O::from(EthereumOrigin::EthereumTransaction(Default::default()))
	}
}
