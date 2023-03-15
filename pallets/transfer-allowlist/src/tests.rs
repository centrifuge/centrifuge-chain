use frame_support::{assert_err, assert_noop, assert_ok, error::BadOrigin};
use frame_system::ensure_signed;
use hex::FromHex;
use pallet_connectors::DomainAddress;
use sp_core::H160;
use xcm::{v1::MultiLocation, VersionedMultiLocation};

use super::*;
use crate::mock::*;

const SENDER: u64 = 0x1;
const ACCOUNT_RECEIVER: u64 = 0x2;

#[test]
fn from_account_works() {
	new_test_ext().execute_with(|| {
		let a = ensure_signed(RuntimeOrigin::signed(SENDER)).unwrap();
		let l: Location<Runtime> = Location::<Runtime>::from(AccountWrapper(a));
		assert_eq!(l, Location::Local(a))
	});
}
#[test]
fn from_xcm_address_works() {
	new_test_ext().execute_with(|| {
		let xa = MultiLocation::default();
		let l = Location::<Runtime>::from(xa.clone());
		assert_eq!(l, Location::XCMV1(MultiLocation::default()))
	});
}
#[test]
fn from_domain_address_works() {
	new_test_ext().execute_with(|| {
		let da = DomainAddress::EVM(
			1284,
			<[u8; 20]>::from_hex("1231231231231231231231231231231231231231").expect(""),
		);
		let l = Location::<Runtime>::from(da.clone());

		assert_eq!(l, Location::Address(da))
	});
}

#[test]
fn add_transfer_allowance_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(ACCOUNT_RECEIVER).into(),
		));
		assert_eq!(
			TransferAllowList::sender_currency_reciever_allowance((
				SENDER,
				CurrencyId::A,
				Location::Local(ACCOUNT_RECEIVER)
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
			RuntimeEvent::TransferAllowList(pallet::Event::TransferAllowanceCreated {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				receiver: Location::Local(ACCOUNT_RECEIVER),
				allowed_at: 0,
				blocked_at: u64::MAX
			})
		)
	})
}

#[test]
fn add_transfer_allowance_fails_if_already_exists() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(ACCOUNT_RECEIVER).into(),
		));
		assert_noop!(
			TransferAllowList::add_transfer_allowance(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A,
				AccountWrapper(ACCOUNT_RECEIVER).into(),
			),
			Error::<Runtime>::ConflictingAllowanceSet
		);
	})
}

#[test]
fn add_transfer_allowance_multiple_dests_increments_correctly() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(ACCOUNT_RECEIVER).into(),
		));
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(100u64).into(),
		));
		assert_eq!(
			TransferAllowList::sender_currency_restriction_set(SENDER, CurrencyId::A).unwrap(),
			2
		);
	})
}

#[test]
fn transfer_allowance_allows_correctly_with_allowance_set() {
	new_test_ext().execute_with(|| {
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(ACCOUNT_RECEIVER).into(),
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
			AccountWrapper(ACCOUNT_RECEIVER).into(),
		));
		assert_eq!(
			TransferAllowList::allowance(SENDER.into(), 55u64, CurrencyId::A),
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
			10
		));

		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(ACCOUNT_RECEIVER).into(),
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
			AccountWrapper(ACCOUNT_RECEIVER).into(),
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
		// create allowance to test removal
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(ACCOUNT_RECEIVER).into(),
		));
		// test removal
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(ACCOUNT_RECEIVER).into(),
		));
		// verify removed
		assert_eq!(
			TransferAllowList::sender_currency_reciever_allowance((
				SENDER,
				CurrencyId::A,
				Location::Local(ACCOUNT_RECEIVER)
			)),
			None
		);
		// verify sender/currency allowance tracking decremented/removed
		assert_eq!(
			TransferAllowList::sender_currency_restriction_set(SENDER, CurrencyId::A),
			None
		);
		// verify event sent for removal
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::TransferAllowList(pallet::Event::TransferAllowanceRemoved {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				receiver: Location::Local(ACCOUNT_RECEIVER),
			})
		);
	})
}
#[test]
fn remove_transfer_allowance_non_existant_transfer_allowance() {
	new_test_ext().execute_with(|| {
		// test removal
		assert_noop!(
			TransferAllowList::remove_transfer_allowance(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A,
				AccountWrapper(ACCOUNT_RECEIVER).into(),
			),
			Error::<Runtime>::NoMatchingAllowance
		);
	})
}

#[test]
fn remove_transfer_allowance_when_multiple_present_for_sender_currency_properly_decrements() {
	new_test_ext().execute_with(|| {
		// add multiple entries for sender/currency to test dec
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(ACCOUNT_RECEIVER).into(),
		));
		assert_ok!(TransferAllowList::add_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(100u64).into(),
		));
		// test removal
		assert_ok!(TransferAllowList::remove_transfer_allowance(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			AccountWrapper(ACCOUNT_RECEIVER).into(),
		));
		// verify correct entry removed
		assert_eq!(
			TransferAllowList::sender_currency_reciever_allowance((
				SENDER,
				CurrencyId::A,
				Location::Local(ACCOUNT_RECEIVER)
			)),
			None
		);
		// verify correct entry still present
		assert_eq!(
			TransferAllowList::sender_currency_reciever_allowance((
				SENDER,
				CurrencyId::A,
				Location::Local(100u64)
			))
			.unwrap(),
			AllowanceDetails {
				allowed_at: 0u64,
				blocked_at: 200u64
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
			200
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
			200
		));
		assert_ok!(TransferAllowList::add_or_update_allowance_delay(
			RuntimeOrigin::signed(SENDER),
			CurrencyId::A,
			250
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
			200
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
