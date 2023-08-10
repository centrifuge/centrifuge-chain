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

use frame_support::{assert_noop, assert_ok, dispatch::RawOrigin};
use sp_runtime::{testing::H256, DispatchError::BadOrigin};

use super::*;
use crate::mock::{RuntimeEvent as MockEvent, *};

mod utils {
	use super::*;

	pub fn event_exists<E: Into<MockEvent>>(e: E) {
		let e: MockEvent = e.into();
		assert!(frame_system::Pallet::<Runtime>::events()
			.iter()
			.any(|ev| ev.event == e));
	}
}

use utils::*;

mod add_code_hash {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let account = H160::from_low_u64_be(1);
			let code_hash = H256::from_low_u64_be(2);

			assert_ok!(EthereumAllowList::add_code_hash(
				RuntimeOrigin::root(),
				account,
				code_hash,
			));

			event_exists(Event::<Runtime>::CodeHashAdded { account, code_hash });
		});
	}

	#[test]
	fn invalid_origin() {
		new_test_ext().execute_with(|| {
			let account = H160::from_low_u64_be(1);
			let code_hash = H256::from_low_u64_be(2);

			assert_noop!(
				EthereumAllowList::add_code_hash(RawOrigin::Signed(1).into(), account, code_hash,),
				BadOrigin,
			);
		});
	}

	#[test]
	fn code_hash_already_exists() {
		new_test_ext().execute_with(|| {
			let account = H160::from_low_u64_be(1);
			let code_hash = H256::from_low_u64_be(2);

			assert_ok!(EthereumAllowList::add_code_hash(
				RuntimeOrigin::root(),
				account,
				code_hash,
			));

			assert_noop!(
				EthereumAllowList::add_code_hash(RuntimeOrigin::root(), account, code_hash,),
				Error::<Runtime>::CodeHashAlreadyExists,
			);
		});
	}
}

mod remove_code_hash {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let account = H160::from_low_u64_be(1);
			let code_hash = H256::from_low_u64_be(2);

			assert_ok!(EthereumAllowList::add_code_hash(
				RuntimeOrigin::root(),
				account,
				code_hash,
			));

			assert_ok!(EthereumAllowList::remove_code_hash(
				RuntimeOrigin::root(),
				account,
				code_hash,
			));

			event_exists(Event::<Runtime>::CodeHashRemoved { account, code_hash });
		});
	}

	#[test]
	fn invalid_origin() {
		new_test_ext().execute_with(|| {
			let account = H160::from_low_u64_be(1);
			let code_hash = H256::from_low_u64_be(2);

			assert_noop!(
				EthereumAllowList::remove_code_hash(
					RawOrigin::Signed(1).into(),
					account,
					code_hash,
				),
				BadOrigin,
			);
		});
	}

	#[test]
	fn code_hash_not_found() {
		new_test_ext().execute_with(|| {
			let account = H160::from_low_u64_be(1);
			let code_hash = H256::from_low_u64_be(2);

			assert_noop!(
				EthereumAllowList::remove_code_hash(RuntimeOrigin::root(), account, code_hash,),
				Error::<Runtime>::CodeHashNotFound,
			);
		});
	}
}
