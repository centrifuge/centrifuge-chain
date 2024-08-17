use frame_support::{assert_noop, assert_ok};
use sp_core::crypto::AccountId32;

use crate::{mock::*, pallet::RouterForwarding, ForwardInfo};

mod set_forwarder {
	use sp_runtime::DispatchError;

	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			assert_ok!(LiquidityPoolsForwarder::set_forwarder(
				RuntimeOrigin::root(),
				FORWARD_ROUTER_ID,
				SOURCE_DOMAIN,
				FORWARD_CONTRACT
			));

			assert_eq!(
				RouterForwarding::<Runtime>::get(FORWARD_ROUTER_ID),
				Some(ForwardInfo {
					contract: FORWARD_CONTRACT,
					source_domain: SOURCE_DOMAIN
				})
			);

			System::assert_last_event(RuntimeEvent::LiquidityPoolsForwarder(
				crate::Event::ForwarderSet {
					router_id: FORWARD_ROUTER_ID,
					source_domain: SOURCE_DOMAIN,
					forwarding_contract: FORWARD_CONTRACT,
				},
			));
		})
	}

	#[test]
	fn erroring_out_with_bad_origin() {
		System::externalities().execute_with(|| {
			assert_noop!(
				LiquidityPoolsForwarder::set_forwarder(
					RuntimeOrigin::signed(AccountId32::new([1u8; 32])),
					FORWARD_ROUTER_ID,
					SOURCE_DOMAIN,
					FORWARD_CONTRACT
				),
				DispatchError::BadOrigin
			);

			assert!(RouterForwarding::<Runtime>::get(FORWARD_ROUTER_ID).is_none());
		})
	}
}

mod remove_forwarder {
	use frame_support::assert_noop;
	use sp_runtime::DispatchError;

	use super::*;

	#[test]
	fn success() {
		System::externalities().execute_with(|| {
			assert_ok!(LiquidityPoolsForwarder::set_forwarder(
				RuntimeOrigin::root(),
				FORWARD_ROUTER_ID,
				SOURCE_DOMAIN,
				FORWARD_CONTRACT
			));

			assert_ok!(LiquidityPoolsForwarder::remove_forwarder(
				RuntimeOrigin::root(),
				FORWARD_ROUTER_ID,
			));

			assert!(RouterForwarding::<Runtime>::get(FORWARD_ROUTER_ID).is_none());

			System::assert_last_event(RuntimeEvent::LiquidityPoolsForwarder(
				crate::Event::ForwarderRemoved {
					router_id: FORWARD_ROUTER_ID,
					source_domain: SOURCE_DOMAIN,
					forwarding_contract: FORWARD_CONTRACT,
				},
			));
		})
	}

	#[test]
	fn success_silent() {
		System::externalities().execute_with(|| {
			assert!(RouterForwarding::<Runtime>::get(FORWARD_ROUTER_ID).is_none());
			assert_ok!(LiquidityPoolsForwarder::remove_forwarder(
				RuntimeOrigin::root(),
				FORWARD_ROUTER_ID,
			));
			assert!(RouterForwarding::<Runtime>::get(FORWARD_ROUTER_ID).is_none());
			assert!(System::events().is_empty());
		})
	}

	#[test]
	fn erroring_out_with_bad_origin() {
		System::externalities().execute_with(|| {
			assert_noop!(
				LiquidityPoolsForwarder::set_forwarder(
					RuntimeOrigin::signed(AccountId32::new([1u8; 32])),
					FORWARD_ROUTER_ID,
					SOURCE_DOMAIN,
					FORWARD_CONTRACT
				),
				DispatchError::BadOrigin
			);
		})
	}
}

mod send_message {
	use cfg_traits::liquidity_pools::{LpMessage, MessageSender};

	use super::*;

	fn config_mocks(msg: Message) {
		MockSenderReceiver::mock_send(move |router_id, sender, message| {
			assert_eq!(router_id, FORWARD_ROUTER_ID);
			assert_eq!(sender, FORWARDER_DOMAIN_ADDRESS);
			assert_eq!(&message, &msg.serialize());

			Ok(())
		});
	}

	mod success {
		use super::*;

		#[test]
		fn with_forwarding() {
			System::externalities().execute_with(|| {
				config_mocks(Message::Forward);
				assert_ok!(LiquidityPoolsForwarder::set_forwarder(
					RuntimeOrigin::root(),
					FORWARD_ROUTER_ID,
					SOURCE_DOMAIN,
					FORWARD_CONTRACT
				));

				assert_ok!(<LiquidityPoolsForwarder as MessageSender>::send(
					FORWARD_ROUTER_ID,
					FORWARDER_DOMAIN_ADDRESS,
					Message::NonForward
				));
			});
		}

		#[test]
		fn without_forwarding() {
			System::externalities().execute_with(|| {
				config_mocks(Message::NonForward);

				assert_ok!(<LiquidityPoolsForwarder as MessageSender>::send(
					FORWARD_ROUTER_ID,
					FORWARDER_DOMAIN_ADDRESS,
					Message::NonForward
				));
			});
		}
	}

	mod erroring_out {
		use sp_runtime::DispatchError;

		use super::*;
		use crate::Error;

		const ERROR: DispatchError = DispatchError::Other("Send failed on purpose");

		#[test]
		/// Attempting to send forwarded message with missing forward info
		/// panics in mock because `Message::NonForward` serialization is
		/// expected
		fn with_missing_forward_info() {
			System::externalities().execute_with(|| {
				config_mocks(Message::Forward);

				assert_noop!(
					<LiquidityPoolsForwarder as MessageSender>::send(
						FORWARD_ROUTER_ID,
						FORWARDER_DOMAIN_ADDRESS,
						Message::Forward
					),
					Error::<Runtime>::ForwardInfoNotFound
				);
			});
		}

		#[test]
		#[should_panic]
		/// Attempting to send forwarded message panics here in mock
		/// because `Message::NonForward` serialization is expected
		fn with_expected_non_forward_serialization() {
			System::externalities().execute_with(|| {
				config_mocks(Message::NonForward);
				assert_ok!(LiquidityPoolsForwarder::set_forwarder(
					RuntimeOrigin::root(),
					FORWARD_ROUTER_ID,
					SOURCE_DOMAIN,
					FORWARD_CONTRACT
				));

				assert_ok!(<LiquidityPoolsForwarder as MessageSender>::send(
					FORWARD_ROUTER_ID,
					FORWARDER_DOMAIN_ADDRESS,
					Message::NonForward
				));
			});
		}

		#[test]
		#[should_panic]
		/// Attempting to send non-forwarded message panics here in mock
		/// because `Message::Forward` serialization is expected
		fn with_expected_forward_serialization() {
			System::externalities().execute_with(|| {
				config_mocks(Message::Forward);

				assert_ok!(<LiquidityPoolsForwarder as MessageSender>::send(
					FORWARD_ROUTER_ID,
					FORWARDER_DOMAIN_ADDRESS,
					Message::NonForward
				));
			});
		}

		#[test]
		fn with_nesting() {
			System::externalities().execute_with(|| {
				config_mocks(Message::Forward);
				assert_ok!(LiquidityPoolsForwarder::set_forwarder(
					RuntimeOrigin::root(),
					FORWARD_ROUTER_ID,
					SOURCE_DOMAIN,
					FORWARD_CONTRACT
				));

				assert_noop!(
					<LiquidityPoolsForwarder as MessageSender>::send(
						FORWARD_ROUTER_ID,
						FORWARDER_DOMAIN_ADDRESS,
						Message::Forward
					),
					ERROR_NESTING
				);
			});
		}

		#[test]
		fn non_forward_with_message_receiver_err() {
			System::externalities().execute_with(|| {
				MockSenderReceiver::mock_send(|_, _, _| Err(ERROR));

				assert_noop!(
					<LiquidityPoolsForwarder as MessageSender>::send(
						FORWARD_ROUTER_ID,
						FORWARDER_DOMAIN_ADDRESS,
						Message::NonForward
					),
					ERROR
				);
			});
		}
	}
}

mod receive_message {
	use cfg_traits::liquidity_pools::{LpMessage, MessageReceiver};

	use super::*;

	fn config_mocks() {
		MockSenderReceiver::mock_receive(move |middleware, origin, message| {
			assert_eq!(middleware, FORWARD_ROUTER_ID);
			assert_eq!(origin, FORWARDER_DOMAIN_ADDRESS);
			assert_eq!(&message, &NON_FORWARD_SERIALIZED_MESSAGE_BYTES);
			Ok(())
		});
	}

	mod success {
		use cfg_traits::liquidity_pools::{LpMessage, MessageReceiver};

		use super::*;

		#[test]
		fn with_forwarding() {
			System::externalities().execute_with(|| {
				config_mocks();
				assert_ok!(LiquidityPoolsForwarder::set_forwarder(
					RuntimeOrigin::root(),
					FORWARD_ROUTER_ID,
					SOURCE_DOMAIN,
					FORWARD_CONTRACT
				));

				assert_ok!(<LiquidityPoolsForwarder as MessageReceiver>::receive(
					FORWARD_ROUTER_ID,
					FORWARDER_DOMAIN_ADDRESS,
					Message::Forward.serialize()
				));
			});
		}

		#[test]
		fn without_forwarding() {
			System::externalities().execute_with(|| {
				config_mocks();

				assert_ok!(<LiquidityPoolsForwarder as MessageReceiver>::receive(
					FORWARD_ROUTER_ID,
					FORWARDER_DOMAIN_ADDRESS,
					Message::NonForward.serialize()
				));
			});
		}
	}

	mod erroring_out {
		use sp_runtime::DispatchError;

		use super::*;
		use crate::Error;

		const ERROR: DispatchError = DispatchError::Other("Receive failed on purpose");

		#[test]
		fn with_missing_forward_info() {
			System::externalities().execute_with(|| {
				config_mocks();

				assert_noop!(
					<LiquidityPoolsForwarder as MessageReceiver>::receive(
						FORWARD_ROUTER_ID,
						FORWARDER_DOMAIN_ADDRESS,
						Message::Forward.serialize()
					),
					Error::<Runtime>::ForwardInfoNotFound
				);
			});
		}

		#[test]
		fn forward_with_message_receiver_err() {
			System::externalities().execute_with(|| {
				MockSenderReceiver::mock_receive(|_, _, _| Err(ERROR));
				assert_ok!(LiquidityPoolsForwarder::set_forwarder(
					RuntimeOrigin::root(),
					FORWARD_ROUTER_ID,
					SOURCE_DOMAIN,
					FORWARD_CONTRACT
				));

				assert_noop!(
					<LiquidityPoolsForwarder as MessageReceiver>::receive(
						FORWARD_ROUTER_ID,
						FORWARDER_DOMAIN_ADDRESS,
						Message::Forward.serialize()
					),
					ERROR
				);
			});
		}
		#[test]
		fn non_forward_with_message_receiver_err() {
			System::externalities().execute_with(|| {
				MockSenderReceiver::mock_receive(|_, _, _| Err(ERROR));

				assert_noop!(
					<LiquidityPoolsForwarder as MessageReceiver>::receive(
						FORWARD_ROUTER_ID,
						FORWARDER_DOMAIN_ADDRESS,
						Message::NonForward.serialize()
					),
					ERROR
				);
			});
		}
	}
}
