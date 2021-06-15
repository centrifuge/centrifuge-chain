use frame_support::storage::child::ChildInfo;

pub const MS_PER_DAY: u64 = 86400000;

/// Get days(round up) since epoch given the timestamp in ms
pub fn get_days_since_epoch(ts: u64) -> u32 {
	let remainder = ts % MS_PER_DAY;
	let days = (ts / MS_PER_DAY) as u32;
	if remainder == 0 {
		days
	} else {
		days + 1
	}
}

/// Create a child info from the given specific key
pub fn generate_child_storage_key(storage_key: &[u8]) -> ChildInfo {
	let cf: ChildInfo = ChildInfo::new_default(&storage_key);
	cf
}

#[cfg(test)]
mod tests {
	use crate::common::{generate_child_storage_key, get_days_since_epoch};
	use codec::Encode;
	use frame_support::storage::child::ChildInfo;

	#[test]
	fn test_get_days_since_epoch() {
		// 1971-01-01  00:00:00
		assert_eq!(get_days_since_epoch(31536000000), 365);

		// 1971-01-01  00:00:01
		assert_eq!(get_days_since_epoch(31536001000), 366);

		// 1970-12-31  11:59:59
		assert_eq!(get_days_since_epoch(31449600000), 364);
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
