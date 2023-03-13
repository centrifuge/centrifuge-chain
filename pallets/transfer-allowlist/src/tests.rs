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
			0u64,
			200u64,
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
				blocked_at: 200u64
			}
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::TransferAllowList(pallet::Event::TransferAllowanceCreated {
				sender_account_id: SENDER,
				currency_id: CurrencyId::A,
				receiver: Location::Local(ACCOUNT_RECEIVER),
				allowed_at: 0,
				blocked_at: 200
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
			0u64,
			200u64,
		));
		assert_noop!(
			TransferAllowList::add_transfer_allowance(
				RuntimeOrigin::signed(SENDER),
				CurrencyId::A,
				AccountWrapper(ACCOUNT_RECEIVER).into(),
				0u64,
				200u64,
			),
			Error::<Runtime>::ConflictingAllowanceSet
		);
	})
}
