use cfg_traits::liquidity_pools::MessageQueue as _;
use frame_support::{
	assert_noop, assert_ok, dispatch::RawOrigin, pallet_prelude::Hooks, weights::Weight,
};
use sp_runtime::{traits::BadOrigin, DispatchError};

use crate::{
	mock::{new_test_ext, Processor, Queue, Runtime, RuntimeEvent as MockEvent, RuntimeOrigin},
	Error, Event, FailedMessageQueue, LastProcessedNonce, MessageQueue,
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
			let nonce = 1;

			MessageQueue::<Runtime>::insert(nonce, message);

			Processor::mock_process(move |msg| {
				assert_eq!(msg, message);

				(Ok(()), Default::default())
			});

			assert_ok!(Queue::process_message(RuntimeOrigin::signed(1), nonce));

			assert!(MessageQueue::<Runtime>::get(nonce).is_none());

			event_exists(Event::<Runtime>::MessageExecutionSuccess { nonce, message })
		});
	}

	#[test]
	fn failure_bad_origin() {
		new_test_ext().execute_with(|| {
			assert_noop!(Queue::process_message(RawOrigin::None.into(), 0), BadOrigin);
		});
	}

	#[test]
	fn failure_message_not_found() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Queue::process_message(RuntimeOrigin::signed(1), 0,),
				Error::<Runtime>::MessageNotFound,
			);
		});
	}

	#[test]
	fn failure_message_processor() {
		new_test_ext().execute_with(|| {
			let message = 1;
			let nonce = 1;

			MessageQueue::<Runtime>::insert(nonce, message);

			let error = DispatchError::Unavailable;

			Processor::mock_process(move |msg| {
				assert_eq!(msg, message);

				(Err(error), Default::default())
			});

			assert_ok!(Queue::process_message(RuntimeOrigin::signed(1), nonce));

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
			let nonce = 1;
			let error = DispatchError::Unavailable;

			FailedMessageQueue::<Runtime>::insert(nonce, (message, error));

			Processor::mock_process(move |msg| {
				assert_eq!(msg, message);

				(Ok(()), Default::default())
			});

			assert_ok!(Queue::process_failed_message(
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
				Queue::process_failed_message(RawOrigin::None.into(), 0,),
				BadOrigin,
			);
		});
	}

	#[test]
	fn failure_message_not_found() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Queue::process_failed_message(RuntimeOrigin::signed(1), 0,),
				Error::<Runtime>::MessageNotFound,
			);
		});
	}

	#[test]
	fn failure_message_processor() {
		new_test_ext().execute_with(|| {
			let message = 1;
			let nonce = 1;
			let error = DispatchError::Unavailable;

			FailedMessageQueue::<Runtime>::insert(nonce, (message, error));

			let error = DispatchError::Unavailable;
			Processor::mock_process(move |msg| {
				assert_eq!(msg, message);

				(Err(error), Default::default())
			});

			assert_ok!(Queue::process_failed_message(
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
	use sp_arithmetic::ArithmeticError::Overflow;

	use super::*;
	use crate::MessageNonceStore;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let message = 1;

			assert_ok!(Queue::queue(message));

			let nonce = 1;

			assert_eq!(MessageQueue::<Runtime>::get(nonce), Some(message));

			event_exists(Event::<Runtime>::MessageSubmitted { nonce, message })
		});
	}

	#[test]
	fn error_on_max_nonce() {
		new_test_ext().execute_with(|| {
			let message = 1;

			MessageNonceStore::<Runtime>::set(u64::MAX);

			assert_noop!(Queue::queue(message), Overflow);
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
			(1..=3).for_each(|i| Queue::queue(i * 10).unwrap());

			Processor::mock_max_processing_weight(|_| PROCESS_LIMIT_WEIGHT);
			let handle = Processor::mock_process(|_| (Ok(()), PROCESS_WEIGHT));

			let weight = Queue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(weight, PROCESS_WEIGHT * 3);
			assert_eq!(handle.times(), 3);
			assert_eq!(MessageQueue::<Runtime>::iter().count(), 0);
			assert_eq!(FailedMessageQueue::<Runtime>::iter().count(), 0);
			assert_eq!(LastProcessedNonce::<Runtime>::get(), 3)
		});
	}

	#[test]
	fn not_all_messages_fit_in_the_block() {
		new_test_ext().execute_with(|| {
			(1..=5).for_each(|i| Queue::queue(i * 10).unwrap());

			Processor::mock_max_processing_weight(|_| PROCESS_LIMIT_WEIGHT);
			let handle = Processor::mock_process(|_| (Ok(()), PROCESS_WEIGHT));

			let weight = Queue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(weight, PROCESS_WEIGHT * 4);
			assert_eq!(handle.times(), 4);
			assert_eq!(MessageQueue::<Runtime>::iter().count(), 1);
			assert_eq!(FailedMessageQueue::<Runtime>::iter().count(), 0);

			// Next block

			let weight = Queue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(weight, PROCESS_WEIGHT);
			assert_eq!(handle.times(), 5);
			assert_eq!(MessageQueue::<Runtime>::iter().count(), 0);
			assert_eq!(LastProcessedNonce::<Runtime>::get(), 5)
		});
	}

	#[test]
	fn with_failed_messages() {
		new_test_ext().execute_with(|| {
			(1..=3).for_each(|i| Queue::queue(i * 10).unwrap());

			Processor::mock_max_processing_weight(|_| PROCESS_LIMIT_WEIGHT);
			let handle = Processor::mock_process(|msg| match msg {
				20 => (Err(DispatchError::Unavailable), PROCESS_WEIGHT / 2),
				_ => (Ok(()), PROCESS_WEIGHT),
			});

			let weight = Queue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(weight, PROCESS_WEIGHT * 2 + PROCESS_WEIGHT / 2);
			assert_eq!(handle.times(), 3);
			assert_eq!(MessageQueue::<Runtime>::iter().count(), 0);
			assert_eq!(FailedMessageQueue::<Runtime>::iter().count(), 1);
			assert_eq!(LastProcessedNonce::<Runtime>::get(), 3)
		});
	}

	#[test]
	fn with_no_messages() {
		new_test_ext().execute_with(|| {
			let _ = Queue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(LastProcessedNonce::<Runtime>::get(), 0)
		});
	}

	#[test]
	fn with_skipped_message_nonce() {
		new_test_ext().execute_with(|| {
			(1..=3).for_each(|i| Queue::queue(i * 10).unwrap());

			Processor::mock_max_processing_weight(|_| PROCESS_LIMIT_WEIGHT);
			let handle = Processor::mock_process(|_| (Ok(()), PROCESS_WEIGHT));

			// Manually process the 2nd nonce, the on_idle hook should skip it and process
			// the remaining nonces.
			assert_ok!(Queue::process_message(RuntimeOrigin::signed(1), 2));

			let weight = Queue::on_idle(0, TOTAL_WEIGHT);

			assert_eq!(weight, PROCESS_WEIGHT * 2);
			assert_eq!(handle.times(), 3);
			assert_eq!(MessageQueue::<Runtime>::iter().count(), 0);
			assert_eq!(FailedMessageQueue::<Runtime>::iter().count(), 0);
			assert_eq!(LastProcessedNonce::<Runtime>::get(), 3)
		});
	}

	#[test]
	fn max_messages() {
		new_test_ext().execute_with(|| {
			LastProcessedNonce::<Runtime>::set(u64::MAX);

			let _ = Queue::on_idle(0, TOTAL_WEIGHT);

			event_exists(Event::<Runtime>::MaxNumberOfMessagesWasReached {
				last_processed_nonce: u64::MAX,
			})
		});
	}
}
