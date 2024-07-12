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

// Rewards-related constants
pub mod rewards {
	use cfg_primitives::{Balance, CFG};

	/// The default amount of stake for
	/// CurrencyId::Staking(StakingCurrency::BlockRewards) which is inherently
	/// assigned to any member of the only group in block rewards.
	pub const DEFAULT_COLLATOR_STAKE: Balance = CFG;
}

pub mod liquidity_pools {
	/// The account id of the solidity restriction manager interface required
	/// for the `hook` param of the `AddTranche` LP message.
	///
	/// NOTE: Temporarily hardcoded.
	pub const SOLIDITY_RESTRICTION_MANAGER_ADDRESS: [u8; 32] =
		hex_literal::hex!("96ffc875c1fb9d072c6357920b27e894d2bac2ac000000000000000045564d00");
}
