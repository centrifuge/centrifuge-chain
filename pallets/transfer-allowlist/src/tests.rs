use cfg_types::locations::Location;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::Header;

use super::*;
use crate::mock::*;

#[test]
fn add_transfer_allowance_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(ACCOUNT_RECEIVER)
			))
			.unwrap(),
			AllowanceDetails {
				allowed_at: 0u64,
				blocked_at: u64::MAX,
			}
		);
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 1,
				current_delay: None,
				once_modifiable_after: None
			})
		);

		// note: event 0 is in new_ext_test setup -- fee key setup
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved { who: 1, amount: 10 })
		);
		assert_eq!(Balances::reserved_balance(&SENDER), 10);
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::TransferAllowList(pallet::Event::TransferAllowanceCreated {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				receiver: Location::TestLocal(ACCOUNT_RECEIVER),
				allowed_at: 0,
				blocked_at: u64::MAX
			})
		)
	})
}

#[test]
fn add_transfer_allowance_updates_with_delay_set() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200
		));
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		),);

		// only one allowance has been created, should still only have 1 reserve
		assert_eq!(Balances::reserved_balance(&SENDER), 10);
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(ACCOUNT_RECEIVER)
			))
			.unwrap(),
			AllowanceDetails {
				// current block is set to 50, delay is 200
				allowed_at: 250u64,
				blocked_at: u64::MAX,
			}
		);
		// verify correctly incremented -- should still just have one val
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A)
				.unwrap(),
			AllowanceMetadata {
				allowance_count: 1,
				current_delay: Some(200u64),
				once_modifiable_after: None
			}
		);
	})
}

#[test]
fn add_transfer_allowance_multiple_dests_increments_correctly() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(Balances::reserved_balance(&SENDER), 10);
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			100u64.into(),
		));
		// verify reserve incremented for second allowance
		assert_eq!(Balances::reserved_balance(&SENDER), 20);
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 2,
				current_delay: None,
				once_modifiable_after: None
			})
		);

		// note: event 0 is in new_ext_test setup -- fee key setup
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved { who: 1, amount: 10 })
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved { who: 1, amount: 10 })
		);
	})
}

#[test]
fn transfer_allowance_allows_correctly_with_allowance_set() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::allowance(SENDER.into(), ACCOUNT_RECEIVER.into(), CurrencyId::A),
			Ok(true)
		)
	})
}

#[test]
fn transfer_allowance_blocks_when_account_not_allowed() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::allowance(SENDER.into(), 55u64.into(), CurrencyId::A),
			Ok(false)
		)
	})
}

#[test]
fn transfer_allowance_blocks_correctly_when_before_start_block() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			10u64.into()
		));

		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::allowance(SENDER.into(), ACCOUNT_RECEIVER.into(), CurrencyId::A),
			Ok(false)
		)
	})
}

#[test]
fn transfer_allowance_blocks_correctly_when_after_blocked_at_block() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::allowance(SENDER.into(), ACCOUNT_RECEIVER.into(), CurrencyId::A),
			Ok(true)
		)
	})
}

#[test]
fn remove_transfer_allowance_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		// ensure blocked at set to restrict transfers
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(ACCOUNT_RECEIVER)
			))
			.unwrap(),
			AllowanceDetails {
				// current block is 50, no delay set
				allowed_at: 0u64,
				blocked_at: 50u64,
			}
		);

		// ensure reserve still in place as we have the in storage
		// merely ensuring transfers blocked
		assert_eq!(Balances::reserved_balance(&SENDER), 10);

		// ensure allowlist entry still in place, just with restricted params
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 1,
				current_delay: None,
				once_modifiable_after: None
			})
		);

		// event 0 - reserve for allowance creation, 1, allowance creation itelf
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::TransferAllowList(pallet::Event::TransferAllowanceRemoved {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				receiver: Location::TestLocal(ACCOUNT_RECEIVER),
				allowed_at: 0u64,
				blocked_at: 50u64
			})
		)
	})
}

#[test]
fn remove_transfer_allowance_with_delay_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64.into()
		));
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(ACCOUNT_RECEIVER)
			))
			.unwrap(),
			AllowanceDetails {
				// current block is 50, no delay set
				allowed_at: 0u64,
				blocked_at: 250u64,
			}
		);

		// ensure only 1 transfer allowlist for sender/currency still in place
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 1,
				current_delay: Some(200u64),
				once_modifiable_after: None
			})
		);

		// ensure only 1 reserve as we've still just got 1 allowance in storage
		assert_eq!(Balances::reserved_balance(&SENDER), 10);

		// event 0 - reserve for allowance creation,
		// 1, allowance creation itself
		// 2, delay creation
		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::TransferAllowList(pallet::Event::TransferAllowanceRemoved {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				receiver: Location::TestLocal(ACCOUNT_RECEIVER),
				allowed_at: 0u64,
				blocked_at: 250u64
			})
		)
	})
}

#[test]
fn purge_transfer_allowance_works() {
	new_test_ext().execute_with(|| {
		// Add delay to ensure blocked_at is not set to MAX
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			5u64
		));
		// create allowance to test removal
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(Balances::reserved_balance(&SENDER), 10);
		advance_n_blocks(6u64);

		// test removal
		assert_ok!(TransferAllowList::purge_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		// verify removed
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(ACCOUNT_RECEIVER)
			)),
			None
		);
		// verify funds released appropriately
		assert_eq!(Balances::reserved_balance(&SENDER), 0);

		// verify sender/currency allowance tracking decremented/removed
		// 5 for delay
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(5u64),
				once_modifiable_after: None
			})
		);
		// verify event sent for removal
		// note: event 0 is in new_ext_test setup -- fee key setup
		// event 1 is delay, addition to ensure blocked at set
		// event 2 is reserve
		// event 3 is allowance creation
		// Event 4 is allowance removal to set blocked at
		// event 5 is unreserve from purge
		// event 6 is purge
		assert_eq!(
			System::events()[5].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved { who: 1, amount: 10 })
		);
		assert_eq!(
			System::events()[6].event,
			RuntimeEvent::TransferAllowList(pallet::Event::TransferAllowancePurged {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				receiver: Location::TestLocal(ACCOUNT_RECEIVER),
			})
		);
	})
}
#[test]
fn purge_transfer_allowance_non_existant_transfer_allowance() {
	new_test_ext().execute_with(|| {
		// test removal
		assert_noop!(
			TransferAllowList::purge_transfer_allowance(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A,
				ACCOUNT_RECEIVER.into(),
			),
			Error::<Runtime>::NoMatchingAllowance
		);
	})
}

#[test]
fn purge_transfer_allowance_when_multiple_present_for_sender_currency_properly_decrements() {
	new_test_ext().execute_with(|| {
		// Add delay to ensure blocked_at is not set to MAX
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			5u64
		));
		// add multiple entries for sender/currency to test dec
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(Balances::reserved_balance(&SENDER), 10);

		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			100u64.into(),
		));
		assert_eq!(Balances::reserved_balance(&SENDER), 20);
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));

		advance_n_blocks(6u64);

		// test removal
		assert_ok!(TransferAllowList::purge_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));

		// verify correct reserve decrement
		assert_eq!(Balances::reserved_balance(&SENDER), 10);

		// verify correct entry removed
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(ACCOUNT_RECEIVER)
			)),
			None
		);

		// verify correct entry still present
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(100u64)
			))
			.unwrap(),
			AllowanceDetails {
				allowed_at: 55u64,
				blocked_at: u64::MAX
			}
		);

		// verify correct decrement
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 1,
				current_delay: Some(5u64),
				once_modifiable_after: None
			})
		);
	})
}

#[test]
fn add_allowance_delay_works() {
	new_test_ext().execute_with(|| {
		// verify extrinsic execution returns ok
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		// verify val in storage
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(200u64),
				once_modifiable_after: None
			})
		);
		// verify event deposited
		// note: event 0 is in new_ext_test setup -- fee key setup
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelayAdd {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				delay: 200
			})
		)
	})
}

#[test]
fn cannot_create_conflicint_allowance_delays() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		assert_noop!(
			TransferAllowList::add_allowance_delay(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A,
				250u64
			),
			Error::<Runtime>::DuplicateDelay
		);
		// verify val in storage
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(200u64),
				once_modifiable_after: None
			})
		);
		// note: event 0 is in new_ext_test setup -- fee key setup
		// verify event deposited
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelayAdd {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				delay: 200
			})
		)
	})
}

#[test]
fn set_allowance_delay_future_modifiable_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A
			)
		);

		// verify val in storage
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(200u64),
				once_modifiable_after: Some(250u64)
			})
		);

		// note:
		// event 0 is in new_ext_test setup -- fee key setup
		// event 1 is delay creation
		// verify event deposited
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::TransferAllowList(Event::ToggleTransferAllowanceDelayFutureModifiable {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				modifiable_once_after: Some(250)
			})
		)
	})
}

#[test]
fn set_allowance_delay_future_modifiable_fails_if_modifiable_set_and_not_reached() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A
			)
		);
		advance_n_blocks(20);

		assert_noop!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A
			),
			Error::<Runtime>::DelayUnmodifiable
		);
	})
}

#[test]
fn set_allowance_delay_future_modifiable_works_if_modifiable_set_and_reached() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A
			)
		);
		advance_n_blocks(200);

		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A
			)
		);
		// verify val in storage
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(200u64),
				once_modifiable_after: None
			})
		);

		// note:
		// event 0 is in new_ext_test setup -- fee key setup
		// event 1 is delay creation
		// event 2 is initial set modifiable
		// verify event deposited
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::TransferAllowList(Event::ToggleTransferAllowanceDelayFutureModifiable {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				modifiable_once_after: None
			})
		)
	})
}

#[test]
fn purge_allowance_delay_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A
			)
		);
		advance_n_blocks(201);
		assert_ok!(TransferAllowList::purge_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A
		));

		// note:
		// event 0 is in new_ext_test setup -- fee key setup
		// event 1 is delay creation
		// event 2 is initial set modifiable
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelayPurge {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
			})
		);

		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(SENDER, CurrencyId::A),
			None
		)
	})
}

#[test]
fn purge_allowance_delay_fails_if_not_set_modifiable() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		assert_noop!(
			TransferAllowList::purge_allowance_delay(RuntimeOrigin::signed(SENDER), CurrencyId::A),
			Error::<Runtime>::DelayUnmodifiable
		);
	})
}

#[test]
fn purge_allowance_delay_fails_if_modifiable_at_not_reached() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		// verify can't be removed before setting future modifiable
		assert_noop!(
			TransferAllowList::purge_allowance_delay(RuntimeOrigin::signed(SENDER), CurrencyId::A),
			Error::<Runtime>::DelayUnmodifiable
		);
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A
			)
		);
		// verify can't remove before modifiable_at reached
		assert_noop!(
			TransferAllowList::purge_allowance_delay(RuntimeOrigin::signed(SENDER), CurrencyId::A),
			Error::<Runtime>::DelayUnmodifiable
		);
		advance_n_blocks(20u64);
		assert_noop!(
			TransferAllowList::purge_allowance_delay(RuntimeOrigin::signed(SENDER), CurrencyId::A),
			Error::<Runtime>::DelayUnmodifiable
		);
	})
}

fn advance_n_blocks(n: u64) {
	match n {
		n if n > 0 => {
			let h = System::finalize();
			let b = h.number.checked_add(1).unwrap();
			System::initialize(&b.into(), h.parent_hash(), h.digest());
			advance_n_blocks(n - 1);
		}
		_ => (),
	}
}
