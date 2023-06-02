use frame_support::{assert_err, assert_noop, assert_ok};
use sp_core::H256;

use super::*;
use crate::mock::*;

#[test]
fn hash_works() {
	new_test_ext().execute_with(|| {
		let expected: H256 = [
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 124, 26, 16, 236, 141, 56, 42, 126, 225, 64, 28, 191, 37,
			51, 131, 63, 224, 233, 24, 207, 211, 182,
		]
		.into();
		assert_eq!(
			OrderBook::gen_hash(ORDER_PLACER_0, CurrencyId::B, CurrencyId::C),
			expected
		)
	})
}
