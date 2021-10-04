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

/// Weight functions needed for Fees.
pub trait WeightInfo {
	fn pre_commit() -> Weight;
	fn commit() -> Weight;
	fn evict_pre_commits() -> Weight;
	fn evict_anchors() -> Weight;
}

impl WeightInfo for () {
	fn pre_commit() -> Weight {
		193_000_000
	}

	fn commit() -> Weight {
		190_000_000
	}

	fn evict_pre_commits() -> Weight {
		192_000_000
	}

	fn evict_anchors() -> Weight {
		195_000_000
	}
}
