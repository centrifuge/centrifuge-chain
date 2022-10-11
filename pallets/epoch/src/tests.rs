use super::*;
use crate::mock::*;

const NEW_ASSOCIATED_DATA: u32 = 5;

#[test]
fn first_default_epoch() {
	let expected = EpochDetails {
		ends_on: INITIAL_BLOCK,
		associated_data: u32::default(),
	};

	new_test_ext().execute_with(|| {
		assert_eq!(
			Epoch1::update_next_associated_data(|associated_data| -> Result<(), ()> {
				*associated_data = NEW_ASSOCIATED_DATA;
				Ok(())
			}),
			Ok(())
		);
		assert_eq!(ActiveEpoch::<Test, Instance1>::get(), expected);
		assert_eq!(
			Epoch1::update_epoch(|epoch| {
				assert_eq!(epoch, &expected);
				23
			}),
			Some(23)
		);
	});
}

#[test]
fn epoch_after_first_default_epoch() {
	let expected = EpochDetails {
		ends_on: INITIAL_BLOCK + EPOCH_1_PERIOD,
		associated_data: NEW_ASSOCIATED_DATA,
	};

	new_test_ext().execute_with(|| {
		assert_eq!(
			Epoch1::update_next_associated_data(|associated_data| -> Result<(), ()> {
				*associated_data = NEW_ASSOCIATED_DATA;
				Ok(())
			}),
			Ok(())
		);
		assert_eq!(Epoch1::update_epoch(|_| ()), Some(()));
		assert_eq!(ActiveEpoch::<Test, Instance1>::get(), expected);

		mock::advance_in_time(EPOCH_1_PERIOD);
		assert_eq!(
			Epoch1::update_epoch(|epoch| {
				assert_eq!(epoch, &expected);
			}),
			Some(())
		);
	});
}

#[test]
fn epoch_after_two_epochs() {
	let expected = EpochDetails {
		ends_on: INITIAL_BLOCK + EPOCH_1_PERIOD + EPOCH_1_PERIOD,
		associated_data: NEW_ASSOCIATED_DATA,
	};

	new_test_ext().execute_with(|| {
		assert_eq!(
			Epoch1::update_next_associated_data(|associated_data| -> Result<(), ()> {
				*associated_data = NEW_ASSOCIATED_DATA;
				Ok(())
			}),
			Ok(())
		);
		assert_eq!(Epoch1::update_epoch(|_| ()), Some(()));

		mock::advance_in_time(EPOCH_1_PERIOD);
		assert_eq!(Epoch1::update_epoch(|_| ()), Some(()));
		assert_eq!(ActiveEpoch::<Test, Instance1>::get(), expected);

		mock::advance_in_time(EPOCH_1_PERIOD);
		assert_eq!(
			Epoch1::update_epoch(|epoch| {
				assert_eq!(epoch, &expected);
			}),
			Some(())
		);
	});
}

#[test]
fn associated_data_not_updated_if_fails() {
	new_test_ext().execute_with(|| {
		assert_eq!(
			Epoch1::update_next_associated_data(|associated_data| -> Result<(), ()> {
				*associated_data = NEW_ASSOCIATED_DATA;
				Err(())
			}),
			Err(())
		);
		assert_eq!(Epoch1::update_epoch(|_| ()), Some(()));

		mock::advance_in_time(EPOCH_1_PERIOD);
		assert_eq!(
			Epoch1::update_epoch(|epoch| {
				assert_eq!(epoch.associated_data, u32::default());
			}),
			Some(())
		);
	});
}

#[test]
fn callback_only_called_when_epoch_changes() {
	new_test_ext().execute_with(|| {
		assert_eq!(Epoch1::update_epoch(|_| ()), Some(()));

		mock::advance_in_time(EPOCH_1_PERIOD / 2);
		assert_eq!(Epoch1::update_epoch(|_| ()), None);

		mock::advance_in_time(EPOCH_1_PERIOD / 2);
		assert_eq!(Epoch1::update_epoch(|_| ()), Some(()));
	});
}
