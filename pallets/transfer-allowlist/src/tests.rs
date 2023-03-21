use cfg_types::{domain_address::DomainAddress, locations::Location};
use frame_support::{assert_noop, assert_ok};
use frame_system::ensure_signed;
use hex::FromHex;
use xcm::{v1::MultiLocation, VersionedMultiLocation};

use super::*;
use crate::mock::*;

#[test]
fn from_test_account_works() {
	new_test_ext().execute_with(|| {
		let a = ensure_signed(RuntimeOrigin::signed(SENDER)).unwrap();
		let l: Location = Location::from(a);
		assert_eq!(l, Location::TestLocal(a))
	});
}
#[test]
fn from_xcm_v1_address_works() {
	new_test_ext().execute_with(|| {
		let xa = MultiLocation::default();
		let l = Location::from(xa.clone());
		assert_eq!(
			l,
			Location::XCM(sp_core::H256(
				<[u8; 32]>::from_hex(
					"9ee6dfb61a2fb903df487c401663825643bb825d41695e63df8af6162ab145a6"
				)
				.unwrap()
			))
		);
	});
}

#[test]
fn from_xcm_versioned_address_works() {
	let xa = VersionedMultiLocation::V1(MultiLocation::default());
	let l = Location::from(xa.clone());
	assert_eq!(
		l,
		Location::XCM(sp_core::H256(
			<[u8; 32]>::from_hex(
				"5a121beb1148b31fc56f3d26f80800fd9eb4a90435a72d3cc74c42bc72bca9b8"
			)
			.unwrap()
		))
	);
}
#[test]
fn from_domain_address_works() {
	new_test_ext().execute_with(|| {
		let da = DomainAddress::EVM(
			1284,
			<[u8; 20]>::from_hex("1231231231231231231231231231231231231231").unwrap(),
		);
		let l = Location::from(da.clone());

		assert_eq!(l, Location::Address(da))
	});
}

#[test]
fn add_transfer_allowance_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(
			TransferAllowList::sender_currency_reciever_allowance((
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
			TransferAllowList::sender_currency_restriction_set(SENDER, CurrencyId::A).unwrap(),
			1
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved { who: 1, amount: 10 })
		);
		assert_eq!(Balances::reserved_balance(&SENDER), 10);
		assert_eq!(
			System::events()[1].event,
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
		assert_ok!(TransferAllowList::add_or_update_allowance_delay(
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
			TransferAllowList::sender_currency_reciever_allowance((
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
			TransferAllowList::sender_currency_restriction_set(SENDER, CurrencyId::A).unwrap(),
			1
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
			TransferAllowList::sender_currency_restriction_set(SENDER, CurrencyId::A).unwrap(),
			2
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
		assert_ok!(TransferAllowList::add_or_update_allowance_delay(
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
			TransferAllowList::sender_currency_reciever_allowance((
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
			TransferAllowList::sender_currency_restriction_set(SENDER, CurrencyId::A),
			Some(1)
		);

		// event 0 - reserve for allowance creation, 1, allowance creation itelf
		assert_eq!(
			System::events()[2].event,
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
		assert_ok!(TransferAllowList::add_or_update_allowance_delay(
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
			TransferAllowList::sender_currency_reciever_allowance((
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
			TransferAllowList::sender_currency_restriction_set(SENDER, CurrencyId::A),
			Some(1)
		);

		// ensure only 1 reserve as we've still just got 1 allowance in storage
		assert_eq!(Balances::reserved_balance(&SENDER), 10);

		// event 0 - reserve for allowance creation,
		// 1, allowance creation itself
		// 2, delay creation
		assert_eq!(
			System::events()[3].event,
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
		// create allowance to test removal
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		assert_eq!(Balances::reserved_balance(&SENDER), 10);

		// test removal
		assert_ok!(TransferAllowList::purge_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			ACCOUNT_RECEIVER.into(),
		));
		// verify removed
		assert_eq!(
			TransferAllowList::sender_currency_reciever_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(ACCOUNT_RECEIVER)
			)),
			None
		);
		// verify funds released appropriately
		assert_eq!(Balances::reserved_balance(&SENDER), 0);

		// verify sender/currency allowance tracking decremented/removed
		assert_eq!(
			TransferAllowList::sender_currency_restriction_set(SENDER, CurrencyId::A),
			None
		);
		// verify event sent for removal
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved { who: 1, amount: 10 })
		);
		assert_eq!(
			System::events()[3].event,
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
			TransferAllowList::sender_currency_reciever_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(ACCOUNT_RECEIVER)
			)),
			None
		);

		// verify correct entry still present
		assert_eq!(
			TransferAllowList::sender_currency_reciever_allowance((
				SENDER,
				CurrencyId::A,
				Location::TestLocal(100u64)
			))
			.unwrap(),
			AllowanceDetails {
				allowed_at: 0u64,
				blocked_at: u64::MAX
			}
		);

		// verify correct decrement
		assert_eq!(
			TransferAllowList::sender_currency_restriction_set(SENDER, CurrencyId::A).unwrap(),
			1
		);
	})
}

#[test]
fn add_allowance_delay_works() {
	new_test_ext().execute_with(|| {
		// verify extrinsic execution returns ok
		assert_ok!(TransferAllowList::add_or_update_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		// verify val in storage
		assert_eq!(
			TransferAllowList::sender_currency_delay(SENDER, CurrencyId::A).unwrap(),
			200
		);
		// verify event deposited
		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelaySet {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				delay: 200
			})
		)
	})
}

#[test]
fn update_allowance_delay_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_or_update_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));
		assert_ok!(TransferAllowList::add_or_update_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			250u64
		));
		// verify val in storage
		assert_eq!(
			TransferAllowList::sender_currency_delay(SENDER, CurrencyId::A).unwrap(),
			250
		);
		// verify event deposited
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelaySet {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				delay: 250
			})
		)
	})
}

#[test]
fn remove_allowance_delay_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_or_update_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			200u64
		));

		assert_ok!(TransferAllowList::remove_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
		));
		// verify val in storage
		assert_eq!(
			TransferAllowList::sender_currency_delay(SENDER, CurrencyId::A),
			None
		);
		// verify event deposited
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::TransferAllowList(Event::TransferAllowanceDelayRemoval {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A
			})
		)
	})
}

#[test]
fn remove_allowance_delay_when_no_delay_set() {
	new_test_ext().execute_with(|| {
		// should fail now
		assert_noop!(
			TransferAllowList::remove_allowance_delay(RuntimeOrigin::signed(SENDER), CurrencyId::A,),
			Error::<Runtime>::NoMatchingDelay
		);
		// verify no val in storage
		assert_eq!(
			TransferAllowList::sender_currency_delay(SENDER, CurrencyId::A),
			None
		);
	})
}
