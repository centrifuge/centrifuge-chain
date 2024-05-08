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

use frame_support::storage::child::ChildInfo;

// FIXME (ToZ):
// Will be better to use runtime-common constant instead. However
// there is a circular dependeny between runtime-common and this
// create :( (due to AnchorData struct). Perhaps a good idea to
// move AnchorStruct to runtime-common impl module.
pub const MILLISECS_PER_DAY: u64 = 86400000;

/// Get days(round up) since epoch given the timestamp in ms
pub fn get_days_since_epoch(ts: u64) -> Option<u32> {
	let remainder = ts % MILLISECS_PER_DAY;
	let days = u32::try_from(ts / MILLISECS_PER_DAY).ok()?;
	if remainder == 0 {
		Some(days)
	} else {
		days.checked_add(1)
	}
}

/// Create a child info from the given specific key
pub fn generate_child_storage_key(storage_key: &[u8]) -> ChildInfo {
	let cf: ChildInfo = ChildInfo::new_default(&storage_key);
	cf
}

#[cfg(test)]
mod tests {
	use frame_support::storage::child::ChildInfo;
	use parity_scale_codec::Encode;

	use crate::common::{generate_child_storage_key, get_days_since_epoch};

	#[test]
	fn test_get_days_since_epoch() {
		// 1971-01-01  00:00:00
		assert_eq!(get_days_since_epoch(31536000000), Some(365));

		// 1971-01-01  00:00:01
		assert_eq!(get_days_since_epoch(31536001000), Some(366));

		// 1970-12-31  11:59:59
		assert_eq!(get_days_since_epoch(31449600000), Some(364));

		// Overflow Test with MAX u32 (after division)
		assert_eq!(get_days_since_epoch(371085174374358017), None);
	}

	#[test]
	fn test_child_storage_key() {
		let mut expected: &[u8] = &[1, 0, 0, 0];

		assert_eq!(
			generate_child_storage_key(&1.encode()),
			ChildInfo::new_default(expected)
		);

		let prefix = b"anchor";
		expected = &[97, 110, 99, 104, 111, 114, 1, 0, 0, 0];
		let mut prefixed_key = Vec::with_capacity(prefix.len() + 4);
		prefixed_key.extend_from_slice(prefix);
		prefixed_key.extend_from_slice(&1.encode());

		assert_eq!(
			generate_child_storage_key(&prefixed_key),
			ChildInfo::new_default(expected)
		);
	}
}
