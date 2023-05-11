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

// Pools-related constants
pub mod pools {
	use frame_support::parameter_types;
	use scale_info::TypeInfo;

	parameter_types! {
		/// The max length in bytes allowed for a tranche token name
		#[derive(TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
		pub const MaxTrancheNameLengthBytes: u32 = 128;

		/// The max length in bytes allowed for a tranche token symbol
		#[derive(TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
		pub const MaxTrancheSymbolLengthBytes: u32 = 32;
	}
}

// Rewards-related constants
pub mod rewards {
	use cfg_primitives::{Balance, CFG};

	/// The default amount of stake for
	/// CurrencyId::Staking(StakingCurrency::BlockRewards) which is inherently
	/// assigned to any member of the only group in block rewards.
	pub const DEFAULT_COLLATOR_STAKE: Balance = CFG;
}
