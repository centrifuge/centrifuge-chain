// Copyright 2023 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::fee::PoolFeeBucket;
use frame_support::dispatch::TypeInfo;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};

use crate::{Config, PoolFeeInfoOf};

/// Represents pool changes which might require to complete further guarding
/// checks.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
#[scale_info(skip_type_params(T))]
pub enum Change<T: Config> {
	AppendFee(PoolFeeBucket, PoolFeeInfoOf<T>),
}
