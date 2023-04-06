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

use frame_support::weights::Weight;

pub trait WeightInfo {
	fn claim_reward_ed25519() -> Weight;
	fn claim_reward_sr25519() -> Weight;
	fn claim_reward_ecdsa() -> Weight;
	fn initialize() -> Weight;
	fn set_lease_start() -> Weight;
	fn set_lease_period() -> Weight;
	fn set_contributions_root() -> Weight;
	fn set_locked_at() -> Weight;
	fn set_crowdloan_trie_index() -> Weight;
}

impl WeightInfo for () {
	fn claim_reward_ed25519() -> Weight {
		Weight::zero()
	}

	fn claim_reward_sr25519() -> Weight {
		Weight::zero()
	}

	fn claim_reward_ecdsa() -> Weight {
		Weight::zero()
	}

	fn initialize() -> Weight {
		Weight::zero()
	}

	fn set_lease_start() -> Weight {
		Weight::zero()
	}

	fn set_lease_period() -> Weight {
		Weight::zero()
	}

	fn set_contributions_root() -> Weight {
		Weight::zero()
	}

	fn set_locked_at() -> Weight {
		Weight::zero()
	}

	fn set_crowdloan_trie_index() -> Weight {
		Weight::zero()
	}
}
