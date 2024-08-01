use cfg_primitives::LPGatewayQueueMessageNonce;
use cfg_traits::liquidity_pools::{
	test_util::Message as LPTestMessage, MessageQueue as MessageQueueT,
};
use frame_support::{assert_noop, assert_ok, dispatch::RawOrigin};
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
			let message = LPTestMessage {};
			let nonce = LPGatewayQueueMessageNonce::one();

			MessageQueue::<Runtime>::insert(nonce, message.clone());

			let msg_clone = message.clone();
			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, msg_clone);

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
			let message = LPTestMessage {};
			let nonce = LPGatewayQueueMessageNonce::one();

			MessageQueue::<Runtime>::insert(nonce, message.clone());

			let msg_clone = message.clone();
			let error = DispatchError::Unavailable;

			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, msg_clone);

				(Err(error), Default::default())
			});

			assert_ok!(LPGatewayQueue::process_message(
				RuntimeOrigin::signed(1),
				nonce
			));

			assert_eq!(
				FailedMessageQueue::<Runtime>::get(nonce),
				Some((message.clone(), error))
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
			let message = LPTestMessage {};
			let nonce = LPGatewayQueueMessageNonce::one();
			let error = DispatchError::Unavailable;

			FailedMessageQueue::<Runtime>::insert(nonce, (message.clone(), error));

			let msg_clone = message.clone();
			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, msg_clone);

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
			let message = LPTestMessage {};
			let nonce = LPGatewayQueueMessageNonce::one();
			let error = DispatchError::Unavailable;

			FailedMessageQueue::<Runtime>::insert(nonce, (message.clone(), error));

			let msg_clone = message.clone();
			let error = DispatchError::Unavailable;
			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, msg_clone);

				(Err(error), Default::default())
			});

			assert_ok!(LPGatewayQueue::process_failed_message(
				RuntimeOrigin::signed(1),
				nonce
			));

			assert_eq!(
				FailedMessageQueue::<Runtime>::get(nonce),
				Some((message.clone(), error))
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
			let message = LPTestMessage {};

			assert_ok!(LPGatewayQueue::submit(message.clone()));

			let nonce = LPGatewayQueueMessageNonce::one();

			assert_eq!(MessageQueue::<Runtime>::get(nonce), Some(message.clone()));

			event_exists(Event::<Runtime>::MessageSubmitted { nonce, message })
		});
	}
}
