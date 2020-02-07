use codec::Encode;
use sp_std::vec::Vec;

// default substrate child storage root
const CHILD_STORAGE_DEFAULT_PREFIX: &[u8] = b":child_storage:default:";

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

/// Create a child storage key from the given specific key
pub fn generate_child_storage_key(specific_key: u32) -> Vec<u8> {
    let mut child_storage_key = CHILD_STORAGE_DEFAULT_PREFIX.to_vec();
    child_storage_key.extend_from_slice(&specific_key.encode());
    child_storage_key
}

#[cfg(test)]
mod tests {

    use crate::common::{generate_child_storage_key, get_days_since_epoch};

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
        assert_eq!(
            generate_child_storage_key(1),
            vec![
                58, 99, 104, 105, 108, 100, 95, 115, 116, 111, 114, 97, 103, 101, 58, 100, 101,
                102, 97, 117, 108, 116, 58, 1, 0, 0, 0
            ]
        );
    }
}
