use cfg_primitives::LPGatewayQueueMessageNonce;
use cfg_traits::queue::MessageQueue as _;
use frame_support::{
	assert_noop, assert_ok, dispatch::RawOrigin, pallet_prelude::Hooks, weights::Weight,
};
use sp_runtime::{
	traits::{BadOrigin, One, Zero},
	DispatchError,
};

use crate::{
	mock::{
		new_test_ext, LPGatewayMock, LPGatewayQueue, Runtime, RuntimeEvent as MockEvent,
		RuntimeOrigin,
	},
	Error, Event, FailedMessageQueue, MessageQueue,
};

mod utils {
	use super::*;

	pub fn event_exists<E: Into<MockEvent>>(e: E) {
		let e: MockEvent = e.into();
		let events = frame_system::Pallet::<Runtime>::events();

		assert!(events.iter().any(|ev| ev.event == e));
	}
}

use utils::*;

mod process_message {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let message = 1;
			let nonce = LPGatewayQueueMessageNonce::one();

			MessageQueue::<Runtime>::insert(nonce, message);

			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, message);

				(Ok(()), Default::default())
			});

			assert_ok!(LPGatewayQueue::process_message(
				RuntimeOrigin::signed(1),
				nonce
			));

			assert!(MessageQueue::<Runtime>::get(nonce).is_none());

			event_exists(Event::<Runtime>::MessageExecutionSuccess { nonce, message })
		});
	}

	#[test]
	fn failure_bad_origin() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				LPGatewayQueue::process_message(
					RawOrigin::None.into(),
					LPGatewayQueueMessageNonce::zero(),
				),
				BadOrigin,
			);
		});
	}

	#[test]
	fn failure_message_not_found() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				LPGatewayQueue::process_message(
					RuntimeOrigin::signed(1),
					LPGatewayQueueMessageNonce::zero(),
				),
				Error::<Runtime>::MessageNotFound,
			);
		});
	}

	#[test]
	fn failure_message_processor() {
		new_test_ext().execute_with(|| {
			let message = 1;
			let nonce = LPGatewayQueueMessageNonce::one();

			MessageQueue::<Runtime>::insert(nonce, message);

			let error = DispatchError::Unavailable;

			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, message);

				(Err(error), Default::default())
			});

			assert_ok!(LPGatewayQueue::process_message(
				RuntimeOrigin::signed(1),
				nonce
			));

			assert_eq!(
				FailedMessageQueue::<Runtime>::get(nonce),
				Some((message, error))
			);

			event_exists(Event::<Runtime>::MessageExecutionFailure {
				nonce,
				message,
				error,
			})
		});
	}
}

mod process_failed_message {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let message = 1;
			let nonce = LPGatewayQueueMessageNonce::one();
			let error = DispatchError::Unavailable;

			FailedMessageQueue::<Runtime>::insert(nonce, (message, error));

			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, message);

				(Ok(()), Default::default())
			});

			assert_ok!(LPGatewayQueue::process_failed_message(
				RuntimeOrigin::signed(1),
				nonce
			));

			assert!(FailedMessageQueue::<Runtime>::get(nonce).is_none());

			event_exists(Event::<Runtime>::MessageExecutionSuccess { nonce, message })
		});
	}

	#[test]
	fn failure_bad_origin() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				LPGatewayQueue::process_failed_message(
					RawOrigin::None.into(),
					LPGatewayQueueMessageNonce::zero(),
				),
				BadOrigin,
			);
		});
	}

	#[test]
	fn failure_message_not_found() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				LPGatewayQueue::process_failed_message(
					RuntimeOrigin::signed(1),
					LPGatewayQueueMessageNonce::zero(),
				),
				Error::<Runtime>::MessageNotFound,
			);
		});
	}

	#[test]
	fn failure_message_processor() {
		new_test_ext().execute_with(|| {
			let message = 1;
			let nonce = LPGatewayQueueMessageNonce::one();
			let error = DispatchError::Unavailable;

			FailedMessageQueue::<Runtime>::insert(nonce, (message, error));

			let error = DispatchError::Unavailable;
			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, message);

				(Err(error), Default::default())
			});

			assert_ok!(LPGatewayQueue::process_failed_message(
				RuntimeOrigin::signed(1),
				nonce
			));

			assert_eq!(
				FailedMessageQueue::<Runtime>::get(nonce),
				Some((message, error))
			);

			event_exists(Event::<Runtime>::MessageExecutionFailure {
				nonce,
				message,
				error,
			})
		});
	}
}

mod message_queue_impl {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let message = 1;

			assert_ok!(LPGatewayQueue::submit(message));

			let nonce = LPGatewayQueueMessageNonce::one();

			assert_eq!(MessageQueue::<Runtime>::get(nonce), Some(message));

			event_exists(Event::<Runtime>::MessageSubmitted { nonce, message })
		});
	}
}

mod on_idle {
	use super::*;

	const PROCESS_LIMIT_WEIGHT: Weight = Weight::from_all(2000);
	const PROCESS_WEIGHT: Weight = Weight::from_all(1000);
	const TOTAL_WEIGHT: Weight = PROCESS_WEIGHT.mul(5);

	#[test]
	fn success_all() {
		new_test_ext().execute_with(|| {
			(1..=3).for_each(|i| MessageQueue::<Runtime>::insert(i as u64, i * 10));

			LPGatewayMock::mock_max_processing_weight(|_| PROCESS_LIMIT_WEIGHT);
			let handle = LPGatewayMock::mock_process(|_| (Ok(()), PROCESS_WEIGHT));

			let weight = LPGatewayQueue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(weight, PROCESS_WEIGHT * 3);
			assert_eq!(handle.times(), 3);
			assert_eq!(MessageQueue::<Runtime>::iter().count(), 0);
			assert_eq!(FailedMessageQueue::<Runtime>::iter().count(), 0);
		});
	}

	#[test]
	fn not_all_messages_fit_in_the_block() {
		new_test_ext().execute_with(|| {
			(1..=5).for_each(|i| MessageQueue::<Runtime>::insert(i as u64, i * 10));

			LPGatewayMock::mock_max_processing_weight(|_| PROCESS_LIMIT_WEIGHT);
			let handle = LPGatewayMock::mock_process(|_| (Ok(()), PROCESS_WEIGHT));

			let weight = LPGatewayQueue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(weight, PROCESS_WEIGHT * 4);
			assert_eq!(handle.times(), 4);
			assert_eq!(MessageQueue::<Runtime>::iter().count(), 1);
			assert_eq!(FailedMessageQueue::<Runtime>::iter().count(), 0);

			// Next block

			let weight = LPGatewayQueue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(weight, PROCESS_WEIGHT);
			assert_eq!(handle.times(), 5);
			assert_eq!(MessageQueue::<Runtime>::iter().count(), 0);
		});
	}

	#[test]
	fn with_failed_messages() {
		new_test_ext().execute_with(|| {
			(1..=3).for_each(|i| MessageQueue::<Runtime>::insert(i as u64, i * 10));

			LPGatewayMock::mock_max_processing_weight(|_| PROCESS_LIMIT_WEIGHT);
			let handle = LPGatewayMock::mock_process(|msg| match msg {
				20 => (Err(DispatchError::Unavailable), PROCESS_WEIGHT / 2),
				_ => (Ok(()), PROCESS_WEIGHT),
			});

			let weight = LPGatewayQueue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(weight, PROCESS_WEIGHT * 2 + PROCESS_WEIGHT / 2);
			assert_eq!(handle.times(), 3);
			assert_eq!(MessageQueue::<Runtime>::iter().count(), 0);
			assert_eq!(FailedMessageQueue::<Runtime>::iter().count(), 1);
		});
	}
}
