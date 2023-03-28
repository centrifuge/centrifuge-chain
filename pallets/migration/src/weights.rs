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
	fn migrate_system_account(n: u32) -> Weight;
	fn migrate_balances_issuance() -> Weight;
	fn migrate_vesting_vesting(n: u32) -> Weight;
	fn migrate_proxy_proxies(n: u32) -> Weight;
	fn finalize() -> Weight;
}

impl WeightInfo for () {
	fn finalize() -> Weight {
		Weight::zero()
	}

	fn migrate_system_account(_: u32) -> Weight {
		Weight::zero()
	}

	fn migrate_balances_issuance() -> Weight {
		Weight::zero()
	}

	fn migrate_vesting_vesting(_: u32) -> Weight {
		Weight::zero()
	}

	fn migrate_proxy_proxies(_: u32) -> Weight {
		Weight::zero()
	}
}
