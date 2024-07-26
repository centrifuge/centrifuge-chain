use crate::mock::new_test_ext;
use crate::mock::LPGatewayMock;
use crate::mock::{LPGatewayQueue, Runtime, RuntimeEvent as MockEvent, RuntimeOrigin};
use crate::Error;
use crate::Event;
use crate::FailedMessageQueue;
use crate::MessageQueue;
use cfg_primitives::LPGatewayMessageNonce;
use cfg_traits::liquidity_pools::test_util::Message as LPTestMessage;
use cfg_traits::liquidity_pools::MessageQueue as MessageQueueT;
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::dispatch::PostDispatchInfo;
use frame_support::dispatch::RawOrigin;
use sp_runtime::traits::BadOrigin;
use sp_runtime::traits::One;
use sp_runtime::traits::Zero;
use sp_runtime::{DispatchError, DispatchErrorWithPostInfo};

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
			let nonce = LPGatewayMessageNonce::one();

			MessageQueue::<Runtime>::insert(nonce, message.clone());

			let msg_clone = message.clone();
			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, msg_clone);

				Ok(PostDispatchInfo::default())
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
					LPGatewayMessageNonce::zero(),
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
					LPGatewayMessageNonce::zero(),
				),
				Error::<Runtime>::MessageNotFound,
			);
		});
	}

	#[test]
	fn failure_message_processor() {
		new_test_ext().execute_with(|| {
			let message = LPTestMessage {};
			let nonce = LPGatewayMessageNonce::one();

			MessageQueue::<Runtime>::insert(nonce, message.clone());

			let msg_clone = message.clone();
			let error = DispatchError::Unavailable;

			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, msg_clone);

				Err(DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo::default(),
					error,
				})
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
			let nonce = LPGatewayMessageNonce::one();
			let error = DispatchError::Unavailable;

			FailedMessageQueue::<Runtime>::insert(nonce, (message.clone(), error));

			let msg_clone = message.clone();
			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, msg_clone);

				Ok(PostDispatchInfo::default())
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
					LPGatewayMessageNonce::zero(),
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
					LPGatewayMessageNonce::zero(),
				),
				Error::<Runtime>::MessageNotFound,
			);
		});
	}

	#[test]
	fn failure_message_processor() {
		new_test_ext().execute_with(|| {
			let message = LPTestMessage {};
			let nonce = LPGatewayMessageNonce::one();
			let error = DispatchError::Unavailable;

			FailedMessageQueue::<Runtime>::insert(nonce, (message.clone(), error));

			let msg_clone = message.clone();
			let error = DispatchError::Unavailable;
			LPGatewayMock::mock_process(move |msg| {
				assert_eq!(msg, msg_clone);

				Err(DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo::default(),
					error,
				})
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

			let nonce = LPGatewayMessageNonce::one();

			assert_eq!(MessageQueue::<Runtime>::get(nonce), Some(message.clone()));

			event_exists(Event::<Runtime>::MessageSubmitted { nonce, message })
		});
	}
}
