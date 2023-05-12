use frame_support::{assert_err, assert_noop, assert_ok};

use super::*;
use crate::mock::*;

#[test]
fn add_transfer_allowance_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				<Runtime as Config>::CurrencyId::default(),
				<Runtime as Config>::Location::from(ACCOUNT_RECEIVER)
			))
			.unwrap(),
			AllowanceDetails {
				allowed_at: 0u64,
				blocked_at: u64::MAX,
			}
		);
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 1,
				current_delay: None,
				once_modifiable_after: None
			})
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved { who: 1, amount: 10 })
		);
		assert_eq!(Balances::reserved_balance(&SENDER), 10);
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceCreated {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
				receiver: <Runtime as Config>::Location::from(ACCOUNT_RECEIVER),
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
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			200
		));
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		),);

		// only one allowance has been created, should still only have 1 reserve
		assert_eq!(Balances::reserved_balance(&SENDER), 10);
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				<Runtime as Config>::CurrencyId::default(),
				<Runtime as Config>::Location::from(ACCOUNT_RECEIVER)
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
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			)
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
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(Balances::reserved_balance(&SENDER), 10);
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			100u64.into(),
		));
		// verify reserve incremented for second allowance
		assert_eq!(Balances::reserved_balance(&SENDER), 20);
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 2,
				current_delay: None,
				once_modifiable_after: None
			})
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved { who: 1, amount: 10 })
		);
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved { who: 1, amount: 10 })
		);
	})
}

#[test]
fn transfer_allowance_allows_correctly_with_allowance_set() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::allowance(
				SENDER.into(),
				ACCOUNT_RECEIVER.into(),
				<Runtime as Config>::CurrencyId::default()
			),
			Ok(())
		)
	})
}

#[test]
fn transfer_allowance_blocks_when_account_not_allowed() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_err!(
			TransferAllowList::allowance(
				SENDER.into(),
				55u64.into(),
				<Runtime as Config>::CurrencyId::default()
			),
			Error::<Runtime>::NoAllowanceForDestination,
		)
	})
}

#[test]
fn transfer_allowance_blocks_correctly_when_before_start_block() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			10u64.into()
		));

		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_err!(
			TransferAllowList::allowance(
				SENDER.into(),
				ACCOUNT_RECEIVER.into(),
				<Runtime as Config>::CurrencyId::default()
			),
			Error::<Runtime>::NoAllowanceForDestination,
		)
	})
}

#[test]
fn transfer_allowance_blocks_correctly_when_after_blocked_at_block() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::allowance(
				SENDER.into(),
				ACCOUNT_RECEIVER.into(),
				<Runtime as Config>::CurrencyId::default()
			),
			Ok(())
		)
	})
}

#[test]
fn remove_transfer_allowance_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		// ensure blocked at set to restrict transfers
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				<Runtime as Config>::CurrencyId::default(),
				<Runtime as Config>::Location::from(ACCOUNT_RECEIVER)
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
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 1,
				current_delay: None,
				once_modifiable_after: None
			})
		);

		// event 1 for allowance creation itelf
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceRemoved {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
				receiver: <Runtime as Config>::Location::from(ACCOUNT_RECEIVER),
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
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			200u64.into()
		));
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				<Runtime as Config>::CurrencyId::default(),
				<Runtime as Config>::Location::from(ACCOUNT_RECEIVER)
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
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 1,
				current_delay: Some(200u64),
				once_modifiable_after: None
			})
		);

		// ensure only 1 reserve as we've still just got 1 allowance in storage
		assert_eq!(Balances::reserved_balance(&SENDER), 10);

		// 0, allowance creation itself
		// 1, delay creation
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceRemoved {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
				receiver: <Runtime as Config>::Location::from(ACCOUNT_RECEIVER),
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
			<Runtime as Config>::CurrencyId::default(),
			5u64
		));
		// create allowance to test removal
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(Balances::reserved_balance(&SENDER), 10);
		advance_n_blocks::<Runtime>(6u64);

		// test removal
		assert_ok!(TransferAllowList::purge_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		// verify removed
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				<Runtime as Config>::CurrencyId::default(),
				<Runtime as Config>::Location::from(ACCOUNT_RECEIVER)
			)),
			None
		);
		// verify funds released appropriately
		assert_eq!(Balances::reserved_balance(&SENDER), 0);

		// verify sender/currency allowance tracking decremented/removed
		// 5 for delay
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(5u64),
				once_modifiable_after: None
			})
		);
		// verify event sent for removal
		// event 0 is delay, addition to ensure blocked at set
		// event 1 is reserve
		// event 2 is allowance creation
		// Event 3 is allowance removal to set blocked at
		// event 4 is unreserve from purge
		// event 5 is purge
		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved { who: 1, amount: 10 })
		);
		assert_eq!(
			System::events()[5].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowancePurged {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
				receiver: <Runtime as Config>::Location::from(ACCOUNT_RECEIVER),
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
				<Runtime as Config>::CurrencyId::default(),
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
			<Runtime as Config>::CurrencyId::default(),
			5u64
		));
		// add multiple entries for sender/currency to test dec
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(Balances::reserved_balance(&SENDER), 10);

		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			100u64.into(),
		));
		assert_eq!(Balances::reserved_balance(&SENDER), 20);
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));

		advance_n_blocks::<Runtime>(6u64);

		// test removal
		assert_ok!(TransferAllowList::purge_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			ACCOUNT_RECEIVER.into(),
		));

		// verify correct reserve decrement
		assert_eq!(Balances::reserved_balance(&SENDER), 10);

		// verify correct entry removed
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				<Runtime as Config>::CurrencyId::default(),
				<Runtime as Config>::Location::from(ACCOUNT_RECEIVER)
			)),
			None
		);

		// verify correct entry still present
		assert_eq!(
			TransferAllowList::get_account_currency_transfer_allowance((
				SENDER,
				<Runtime as Config>::CurrencyId::default(),
				<Runtime as Config>::Location::from(100u64)
			))
			.unwrap(),
			AllowanceDetails {
				allowed_at: 55u64,
				blocked_at: u64::MAX
			}
		);

		// verify correct decrement
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
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
			<Runtime as Config>::CurrencyId::default(),
			200u64
		));
		// verify val in storage
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(200u64),
				once_modifiable_after: None
			})
		);
		// verify event deposited
		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelayAdd {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
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
			<Runtime as Config>::CurrencyId::default(),
			200u64
		));
		assert_noop!(
			TransferAllowList::add_allowance_delay(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default(),
				250u64
			),
			Error::<Runtime>::DuplicateDelay
		);
		// verify val in storage
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(200u64),
				once_modifiable_after: None
			})
		);
		// verify event deposited
		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelayAdd {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
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
			<Runtime as Config>::CurrencyId::default(),
			200u64
		));
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			)
		);

		// verify val in storage
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(200u64),
				once_modifiable_after: Some(250u64)
			})
		);

		// note:
		// event 0 is delay creation
		// verify event deposited
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::TransferAllowList(Event::ToggleTransferAllowanceDelayFutureModifiable {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
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
			<Runtime as Config>::CurrencyId::default(),
			200u64
		));
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			)
		);
		advance_n_blocks::<Runtime>(20);

		assert_noop!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
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
			<Runtime as Config>::CurrencyId::default(),
			200u64
		));
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			)
		);
		advance_n_blocks::<Runtime>(200);

		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			)
		);
		// verify val in storage
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(200u64),
				once_modifiable_after: None
			})
		);

		// note:
		// event 0 is delay creation
		// event 1 is initial set modifiable
		// verify event deposited
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::TransferAllowList(Event::ToggleTransferAllowanceDelayFutureModifiable {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
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
			<Runtime as Config>::CurrencyId::default(),
			200u64
		));
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			)
		);
		advance_n_blocks::<Runtime>(201);
		assert_ok!(TransferAllowList::purge_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default()
		));

		// note:
		// event 0 is delay creation
		// event 1 is initial set modifiable
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelayPurge {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
			})
		);

		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			None
		)
	})
}

#[test]
fn purge_allowance_delay_fails_if_not_set_modifiable() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			200u64
		));
		assert_noop!(
			TransferAllowList::purge_allowance_delay(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			),
			Error::<Runtime>::DelayUnmodifiable
		);
	})
}

#[test]
fn purge_allowance_delay_fails_if_modifiable_at_not_reached() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			200u64
		));
		// verify can't be removed before setting future modifiable
		assert_noop!(
			TransferAllowList::purge_allowance_delay(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			),
			Error::<Runtime>::DelayUnmodifiable
		);
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			)
		);
		// verify can't remove before modifiable_at reached
		assert_noop!(
			TransferAllowList::purge_allowance_delay(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			),
			Error::<Runtime>::DelayUnmodifiable
		);
		advance_n_blocks::<Runtime>(20u64);
		assert_noop!(
			TransferAllowList::purge_allowance_delay(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			),
			Error::<Runtime>::DelayUnmodifiable
		);
	})
}

#[test]
fn update_allowance_delay_fails_if_no_delay() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			TransferAllowList::update_allowance_delay(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default(),
				200u64
			),
			Error::<Runtime>::NoMatchingDelay
		);
	})
}

#[test]
fn update_allowance_delay_fails_if_modifiable_after_not_set() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			10u64
		));
		advance_n_blocks::<Runtime>(15);
		assert_noop!(
			TransferAllowList::update_allowance_delay(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default(),
				20
			),
			Error::<Runtime>::DelayUnmodifiable
		);
	})
}

#[test]
fn update_allowance_delay_fails_if_modifiable_after_not_reached() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			20u64
		));
		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			)
		);
		advance_n_blocks::<Runtime>(15);
		assert_noop!(
			TransferAllowList::update_allowance_delay(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default(),
				20
			),
			Error::<Runtime>::DelayUnmodifiable
		);
	})
}

#[test]
fn update_allowance_delay_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			10u64
		));

		assert_ok!(
			TransferAllowList::toggle_allowance_delay_once_future_modifiable(
				RuntimeOrigin::signed(SENDER),
				<Runtime as Config>::CurrencyId::default()
			)
		);
		advance_n_blocks::<Runtime>(12);
		assert_ok!(TransferAllowList::update_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			<Runtime as Config>::CurrencyId::default(),
			20
		));

		// verify val in storage
		assert_eq!(
			TransferAllowList::get_account_currency_restriction_count_delay(
				SENDER,
				<Runtime as Config>::CurrencyId::default()
			),
			Some(AllowanceMetadata {
				allowance_count: 0,
				current_delay: Some(20u64),
				once_modifiable_after: None
			})
		);

		// note:
		// event 0 is delay creation
		// event 1 is initial set modifiable
		// verify event deposited
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelayUpdate {
				sender_account_id: SENDER,
				currency_id: <Runtime as Config>::CurrencyId::default(),
				delay: 20
			})
		)
	})
}
