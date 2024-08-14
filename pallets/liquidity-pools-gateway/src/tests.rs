use std::collections::HashMap;

use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{LPEncoding, MessageProcessor, OutboundMessageHandler};
use cfg_types::domain_address::*;
use frame_support::{assert_err, assert_noop, assert_ok, weights::Weight};
use itertools::Itertools;
use lazy_static::lazy_static;
use parity_scale_codec::MaxEncodedLen;
use sp_arithmetic::ArithmeticError::Overflow;
use sp_core::{bounded::BoundedVec, crypto::AccountId32, ByteArray, H160};
use sp_runtime::{
	DispatchError,
	DispatchError::{Arithmetic, BadOrigin},
};
use sp_std::sync::{
	atomic::{AtomicU32, Ordering},
	Arc,
};

use super::{
	mock::{RuntimeEvent as MockEvent, *},
	origin::*,
	pallet::*,
};
use crate::{message_processing::InboundEntry, GatewayMessage};

mod utils {
	use super::*;

	pub fn get_test_account_id() -> AccountId32 {
		[0u8; 32].into()
	}

	pub fn get_test_hook_bytes() -> [u8; 20] {
		[10u8; 20]
	}

	pub fn event_exists<E: Into<MockEvent>>(e: E) {
		let e: MockEvent = e.into();
		assert!(frame_system::Pallet::<Runtime>::events()
			.iter()
			.any(|ev| ev.event == e));
	}
}

use utils::*;

mod set_routers {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let mut session_id = 1;

			let mut router_ids =
				BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3]).unwrap();

			assert_ok!(LiquidityPoolsGateway::set_routers(
				RuntimeOrigin::root(),
				router_ids.clone(),
			));

			assert_eq!(Routers::<Runtime>::get(), router_ids.clone());
			assert_eq!(SessionIdStore::<Runtime>::get(), session_id);

			event_exists(Event::<Runtime>::RoutersSet {
				router_ids,
				session_id,
			});

			router_ids = BoundedVec::try_from(vec![ROUTER_ID_3, ROUTER_ID_2, ROUTER_ID_1]).unwrap();

			session_id += 1;

			assert_ok!(LiquidityPoolsGateway::set_routers(
				RuntimeOrigin::root(),
				router_ids.clone(),
			));

			assert_eq!(Routers::<Runtime>::get(), router_ids.clone());
			assert_eq!(SessionIdStore::<Runtime>::get(), session_id);

			event_exists(Event::<Runtime>::RoutersSet {
				router_ids,
				session_id,
			});
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				LiquidityPoolsGateway::set_routers(
					RuntimeOrigin::signed(get_test_account_id()),
					BoundedVec::try_from(vec![]).unwrap(),
				),
				BadOrigin
			);

			assert!(Routers::<Runtime>::get().is_empty());
			assert_eq!(SessionIdStore::<Runtime>::get(), 0);
		});
	}

	#[test]
	fn invalid_routers() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				LiquidityPoolsGateway::set_routers(
					RuntimeOrigin::root(),
					BoundedVec::try_from(vec![]).unwrap(),
				),
				Error::<Runtime>::InvalidRouters
			);
		});
	}

	#[test]
	fn session_id_overflow() {
		new_test_ext().execute_with(|| {
			SessionIdStore::<Runtime>::set(u32::MAX);

			assert_noop!(
				LiquidityPoolsGateway::set_routers(
					RuntimeOrigin::root(),
					BoundedVec::try_from(vec![ROUTER_ID_1]).unwrap(),
				),
				Arithmetic(Overflow)
			);
		});
	}
}

mod add_instance {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(Allowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			event_exists(Event::<Runtime>::InstanceAdded {
				instance: domain_address,
			});
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_noop!(
				LiquidityPoolsGateway::add_instance(
					RuntimeOrigin::signed(get_test_account_id()),
					domain_address.clone(),
				),
				BadOrigin
			);

			assert!(!Allowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));
		});
	}

	#[test]
	fn unsupported_domain() {
		new_test_ext().execute_with(|| {
			let domain_address = DomainAddress::Centrifuge(get_test_account_id().into());

			assert_noop!(
				LiquidityPoolsGateway::add_instance(RuntimeOrigin::root(), domain_address.clone()),
				Error::<Runtime>::DomainNotSupported
			);

			assert!(!Allowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));
		});
	}

	#[test]
	fn instance_already_added() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(Allowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			assert_noop!(
				LiquidityPoolsGateway::add_instance(RuntimeOrigin::root(), domain_address),
				Error::<Runtime>::InstanceAlreadyAdded
			);
		});
	}
}

mod remove_instance {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert_ok!(LiquidityPoolsGateway::remove_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(!Allowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			event_exists(Event::<Runtime>::InstanceRemoved {
				instance: domain_address.clone(),
			});
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_noop!(
				LiquidityPoolsGateway::remove_instance(
					RuntimeOrigin::signed(get_test_account_id()),
					domain_address.clone(),
				),
				BadOrigin
			);
		});
	}

	#[test]
	fn instance_not_found() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_noop!(
				LiquidityPoolsGateway::remove_instance(
					RuntimeOrigin::root(),
					domain_address.clone(),
				),
				Error::<Runtime>::UnknownInstance,
			);
		});
	}
}

mod receive_message_domain {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());
			let message = Message::Simple;

			let router_id = ROUTER_ID_1;

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let encoded_msg = message.serialize();

			let gateway_message = GatewayMessage::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
				router_id: router_id.clone(),
			};

			let handler = MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_message| {
				assert_eq!(mock_message, gateway_message);
				Ok(())
			});

			assert_ok!(LiquidityPoolsGateway::receive_message(
				GatewayOrigin::Domain(domain_address).into(),
				router_id,
				BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
			));

			assert_eq!(handler.times(), 1);
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let encoded_msg = Message::Simple.serialize();

			let router_id = ROUTER_ID_1;

			assert_noop!(
				LiquidityPoolsGateway::receive_message(
					RuntimeOrigin::signed(AccountId32::new([0u8; 32])),
					router_id,
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				BadOrigin,
			);
		});
	}

	#[test]
	fn invalid_message_origin() {
		new_test_ext().execute_with(|| {
			let domain_address = DomainAddress::Centrifuge(get_test_account_id().into());
			let encoded_msg = Message::Simple.serialize();
			let router_id = ROUTER_ID_1;

			assert_noop!(
				LiquidityPoolsGateway::receive_message(
					GatewayOrigin::Domain(domain_address).into(),
					router_id,
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				Error::<Runtime>::InvalidMessageOrigin,
			);
		});
	}

	#[test]
	fn unknown_instance() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());
			let encoded_msg = Message::Simple.serialize();
			let router_id = ROUTER_ID_1;

			assert_noop!(
				LiquidityPoolsGateway::receive_message(
					GatewayOrigin::Domain(domain_address).into(),
					router_id,
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				Error::<Runtime>::UnknownInstance,
			);
		});
	}

	#[test]
	fn message_queue_error() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());
			let message = Message::Simple;

			let router_id = ROUTER_ID_1;

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let encoded_msg = message.serialize();

			let err = sp_runtime::DispatchError::from("liquidity_pools error");

			let gateway_message = GatewayMessage::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
				router_id: router_id.clone(),
			};

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_message| {
				assert_eq!(mock_message, gateway_message);
				Err(err)
			});

			assert_noop!(
				LiquidityPoolsGateway::receive_message(
					GatewayOrigin::Domain(domain_address).into(),
					router_id,
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				err,
			);
		});
	}
}

mod outbound_message_handler_impl {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let sender = get_test_account_id();
			let msg = Message::Simple;
			let message_proof = msg.to_message_proof().get_message_proof().unwrap();

			assert_ok!(LiquidityPoolsGateway::set_routers(
				RuntimeOrigin::root(),
				BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3]).unwrap(),
			));

			let handler = MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_msg| {
				match mock_msg {
					GatewayMessage::Inbound { .. } => {
						assert!(false, "expected outbound message")
					}
					GatewayMessage::Outbound {
						sender, message, ..
					} => {
						assert_eq!(sender, <Runtime as Config>::Sender::get());

						match message {
							Message::Proof(p) => {
								assert_eq!(p, message_proof);
							}
							_ => {}
						}
					}
				}

				Ok(())
			});

			assert_ok!(LiquidityPoolsGateway::handle(sender, domain, msg));
			assert_eq!(handler.times(), 3);
		});
	}

	#[test]
	fn domain_not_supported() {
		new_test_ext().execute_with(|| {
			let domain = Domain::Centrifuge;
			let sender = get_test_account_id();
			let msg = Message::Simple;

			assert_noop!(
				LiquidityPoolsGateway::handle(sender, domain, msg),
				Error::<Runtime>::DomainNotSupported
			);
		});
	}

	#[test]
	fn routers_not_found() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let sender = get_test_account_id();
			let msg = Message::Simple;

			assert_noop!(
				LiquidityPoolsGateway::handle(sender, domain, msg),
				Error::<Runtime>::NotEnoughRoutersForDomain
			);
		});
	}

	#[test]
	fn message_queue_error() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let sender = get_test_account_id();
			let msg = Message::Simple;

			assert_ok!(LiquidityPoolsGateway::set_routers(
				RuntimeOrigin::root(),
				BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3]).unwrap(),
			));

			let gateway_message = GatewayMessage::Outbound {
				sender: <Runtime as Config>::Sender::get(),
				message: msg.clone(),
				router_id: ROUTER_ID_1,
			};

			let err = DispatchError::Unavailable;

			let handler = MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_msg| {
				assert_eq!(mock_msg, gateway_message);

				Err(err)
			});

			assert_noop!(LiquidityPoolsGateway::handle(sender, domain, msg), err);
			assert_eq!(handler.times(), 1);
		});
	}
}

mod set_domain_hook {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);

			assert_ok!(LiquidityPoolsGateway::set_domain_hook_address(
				RuntimeOrigin::root(),
				domain,
				get_test_hook_bytes()
			));
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);

			assert_noop!(
				LiquidityPoolsGateway::set_domain_hook_address(
					RuntimeOrigin::signed(AccountId32::new([0u8; 32])),
					domain,
					get_test_hook_bytes()
				),
				BadOrigin
			);
		});
	}

	#[test]
	fn domain_not_supported() {
		new_test_ext().execute_with(|| {
			let domain = Domain::Centrifuge;

			assert_noop!(
				LiquidityPoolsGateway::set_domain_hook_address(
					RuntimeOrigin::root(),
					domain,
					get_test_hook_bytes()
				),
				Error::<Runtime>::DomainNotSupported
			);
		});
	}
}

mod message_processor_impl {
	use super::*;

	mod inbound {
		use super::*;

		#[macro_use]
		mod util {
			use super::*;

			pub fn run_inbound_message_test_suite(suite: InboundMessageTestSuite) {
				let test_routers = suite.routers;

				for test in suite.tests {
					println!("Executing test for - {:?}", test.router_messages);

					new_test_ext().execute_with(|| {
						let session_id = TEST_SESSION_ID;

						Routers::<Runtime>::set(
							BoundedVec::try_from(test_routers.clone()).unwrap(),
						);
						SessionIdStore::<Runtime>::set(
							session_id,
						);

						let handler = MockLiquidityPools::mock_handle(move |_, _| Ok(()));

						for router_message in test.router_messages {
							let gateway_message = GatewayMessage::Inbound {
								domain_address: TEST_DOMAIN_ADDRESS,
								message: router_message.1,
								router_id: router_message.0,
							};

							let (res, _) = LiquidityPoolsGateway::process(gateway_message);
							assert_ok!(res);
						}

						let expected_message_submitted_times =
							test.expected_test_result.message_submitted_times;
						let message_submitted_times = handler.times();

						assert_eq!(
							message_submitted_times,
							expected_message_submitted_times,
							"Expected message to be submitted {expected_message_submitted_times} times, was {message_submitted_times}"
						);

						for expected_storage_entry in
							test.expected_test_result.expected_storage_entries
						{
							let expected_storage_entry_router_id = expected_storage_entry.0;
							let expected_inbound_entry = expected_storage_entry.1;

							let storage_entry = PendingInboundEntries::<Runtime>::get(
								MESSAGE_PROOF, expected_storage_entry_router_id,
							);
							assert_eq!(storage_entry, expected_inbound_entry, "Expected inbound entry {expected_inbound_entry:?}, found {storage_entry:?}");
						}
					});
				}
			}

			/// Used for generating all `RouterMessage` combinations like:
			///
			/// vec![
			/// 	(ROUTER_ID_1, Message::Simple),
			/// 	(ROUTER_ID_1, Message::Simple),
			/// ]
			/// vec![
			/// 	(ROUTER_ID_1, Message::Simple),
			/// 	(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
			/// ]
			/// vec![
			/// 	(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
			/// 	(ROUTER_ID_1, Message::Simple),
			/// ]
			/// vec![
			/// 	(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
			/// 	(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
			/// ]
			pub fn generate_test_combinations<T>(
				t: T,
				count: usize,
			) -> Vec<Vec<<T as IntoIterator>::Item>>
			where
				T: IntoIterator + Clone,
				T::IntoIter: Clone,
				T::Item: Clone,
			{
				std::iter::repeat(t.clone().into_iter())
					.take(count)
					.multi_cartesian_product()
					.collect::<Vec<_>>()
			}

			/// Type used for mapping a message to a router hash.
			pub type RouterMessage = (RouterId, Message);

			/// Type used for aggregating tests for inbound messages.
			pub struct InboundMessageTestSuite {
				pub routers: Vec<RouterId>,
				pub tests: Vec<InboundMessageTest>,
			}

			/// Type used for defining a test which contains a set of
			/// `RouterMessage` combinations and the expected test result.
			pub struct InboundMessageTest {
				pub router_messages: Vec<RouterMessage>,
				pub expected_test_result: ExpectedTestResult,
			}

			/// Type used for defining the number of expected inbound message
			/// submission and the exected storage state.
			#[derive(Clone, Debug)]
			pub struct ExpectedTestResult {
				pub message_submitted_times: u32,
				pub expected_storage_entries: Vec<(RouterId, Option<InboundEntry<Runtime>>)>,
			}

			/// Generates the combinations of `RouterMessage` used when testing,
			/// maps the `ExpectedTestResult` for each and creates the
			/// `InboundMessageTestSuite`.
			pub fn generate_test_suite(
				routers: Vec<RouterId>,
				test_data: Vec<RouterMessage>,
				expected_results: HashMap<Vec<RouterMessage>, ExpectedTestResult>,
				message_count: usize,
			) -> InboundMessageTestSuite {
				let tests = generate_test_combinations(test_data, message_count);

				let tests = tests
					.into_iter()
					.map(|router_messages| {
						let expected_test_result = expected_results
							.get(&router_messages)
							.expect(
								format!("test for {router_messages:?} should be covered").as_str(),
							)
							.clone();

						InboundMessageTest {
							router_messages,
							expected_test_result,
						}
					})
					.collect::<Vec<_>>();

				InboundMessageTestSuite { routers, tests }
			}
		}

		use util::*;

		mod one_router {
			use super::*;

			#[test]
			fn success() {
				new_test_ext().execute_with(|| {
					let message = Message::Simple;
					let message_proof = message.to_message_proof().get_message_proof().unwrap();
					let session_id = 1;
					let domain_address = DomainAddress::EVM(1, [1; 20]);
					let router_id = ROUTER_ID_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						router_id: router_id.clone(),
					};

					Routers::<Runtime>::set(
						BoundedVec::<_, _>::try_from(vec![router_id.clone()]).unwrap(),
					);
					SessionIdStore::<Runtime>::set(session_id);

					let handler = MockLiquidityPools::mock_handle(
						move |mock_domain_address, mock_message| {
							assert_eq!(mock_domain_address, domain_address);
							assert_eq!(mock_message, message);

							Ok(())
						},
					);

					let (res, _) = LiquidityPoolsGateway::process(gateway_message);
					assert_ok!(res);
					assert_eq!(handler.times(), 1);

					assert!(
						PendingInboundEntries::<Runtime>::get(message_proof, router_id).is_none()
					);
				});
			}

			#[test]
			fn multi_router_not_found() {
				new_test_ext().execute_with(|| {
					let message = Message::Simple;
					let domain_address = DomainAddress::EVM(1, [1; 20]);
					let router_hash = ROUTER_ID_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						router_id: router_hash,
					};

					let (res, _) = LiquidityPoolsGateway::process(gateway_message);
					assert_noop!(res, Error::<Runtime>::NotEnoughRoutersForDomain);
				});
			}

			#[test]
			fn unknown_inbound_message_router() {
				new_test_ext().execute_with(|| {
					let message = Message::Simple;
					let session_id = 1;
					let domain_address = DomainAddress::EVM(1, [1; 20]);
					let router_hash = ROUTER_ID_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						// The router stored has a different hash, this should trigger the expected
						// error.
						router_id: ROUTER_ID_2,
					};

					Routers::<Runtime>::set(
						BoundedVec::<_, _>::try_from(vec![router_hash]).unwrap(),
					);
					SessionIdStore::<Runtime>::set(session_id);

					let (res, _) = LiquidityPoolsGateway::process(gateway_message);
					assert_noop!(res, Error::<Runtime>::UnknownRouter);
				});
			}

			#[test]
			fn expected_message_proof_type() {
				new_test_ext().execute_with(|| {
					let message = Message::Simple;
					let message_proof = message.to_message_proof().get_message_proof().unwrap();
					let session_id = 1;
					let domain_address = DomainAddress::EVM(1, [1; 20]);
					let router_id = ROUTER_ID_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						router_id: router_id.clone(),
					};

					Routers::<Runtime>::set(
						BoundedVec::<_, _>::try_from(vec![router_id.clone()]).unwrap(),
					);
					SessionIdStore::<Runtime>::set(session_id);
					PendingInboundEntries::<Runtime>::insert(
						message_proof,
						router_id,
						InboundEntry::<Runtime>::Proof {
							session_id,
							current_count: 0,
						},
					);

					let (res, _) = LiquidityPoolsGateway::process(gateway_message);
					assert_noop!(res, Error::<Runtime>::ExpectedMessageProofType);
				});
			}
		}

		mod two_routers {
			use super::*;

			mod success {
				use super::*;

				lazy_static! {
					static ref TEST_DATA: Vec<RouterMessage> = vec![
						(ROUTER_ID_1, Message::Simple),
						(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
					];
				}

				mod two_messages {
					use super::*;

					const MESSAGE_COUNT: usize = 2;

					#[test]
					fn success() {
						let expected_results: HashMap<Vec<RouterMessage>, ExpectedTestResult> =
							HashMap::from([
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 2,
												}),
											),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(ROUTER_ID_2, None),
										],
									},
								),
							]);

						let suite = generate_test_suite(
							vec![ROUTER_ID_1, ROUTER_ID_2],
							TEST_DATA.clone(),
							expected_results,
							MESSAGE_COUNT,
						);

						run_inbound_message_test_suite(suite);
					}
				}

				mod three_messages {
					use super::*;

					const MESSAGE_COUNT: usize = 3;

					#[test]
					fn success() {
						let expected_results: HashMap<Vec<RouterMessage>, ExpectedTestResult> =
							HashMap::from([
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 3,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 3,
												}),
											),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 1,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 1,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 1,
												}),
											),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 1,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 1,
												}),
											),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 1,
												}),
											),
										],
									},
								),
							]);

						let suite = generate_test_suite(
							vec![ROUTER_ID_1, ROUTER_ID_2],
							TEST_DATA.clone(),
							expected_results,
							MESSAGE_COUNT,
						);

						run_inbound_message_test_suite(suite);
					}
				}

				mod four_messages {
					use super::*;

					const MESSAGE_COUNT: usize = 4;

					#[test]
					fn success() {
						let expected_results: HashMap<Vec<RouterMessage>, ExpectedTestResult> =
							HashMap::from([
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 4,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 4,
												}),
											),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												ROUTER_ID_1,
												Some(InboundEntry::<Runtime>::Message {
													session_id: TEST_SESSION_ID,
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(ROUTER_ID_2, None),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 2,
												}),
											),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 2,
												}),
											),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 2,
												}),
											),
										],
									},
								),
								(
									vec![
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
										(ROUTER_ID_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(ROUTER_ID_1, None),
											(
												ROUTER_ID_2,
												Some(InboundEntry::<Runtime>::Proof {
													session_id: TEST_SESSION_ID,
													current_count: 2,
												}),
											),
										],
									},
								),
							]);

						let suite = generate_test_suite(
							vec![ROUTER_ID_1, ROUTER_ID_2],
							TEST_DATA.clone(),
							expected_results,
							MESSAGE_COUNT,
						);

						run_inbound_message_test_suite(suite);
					}
				}
			}

			mod failure {
				use super::*;

				#[test]
				fn message_expected_from_first_router() {
					new_test_ext().execute_with(|| {
						let session_id = 1;

						Routers::<Runtime>::set(
							BoundedVec::<_, _>::try_from(vec![ROUTER_ID_1, ROUTER_ID_2]).unwrap(),
						);
						SessionIdStore::<Runtime>::set(session_id);

						let gateway_message = GatewayMessage::Inbound {
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Simple,
							router_id: ROUTER_ID_2,
						};

						let (res, _) = LiquidityPoolsGateway::process(gateway_message);
						assert_noop!(res, Error::<Runtime>::MessageExpectedFromFirstRouter);
					});
				}

				#[test]
				fn proof_not_expected_from_first_router() {
					new_test_ext().execute_with(|| {
						let session_id = 1;

						Routers::<Runtime>::set(
							BoundedVec::<_, _>::try_from(vec![ROUTER_ID_1, ROUTER_ID_2]).unwrap(),
						);
						SessionIdStore::<Runtime>::set(session_id);

						let gateway_message = GatewayMessage::Inbound {
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Proof(MESSAGE_PROOF),
							router_id: ROUTER_ID_1,
						};

						let (res, _) = LiquidityPoolsGateway::process(gateway_message);
						assert_noop!(res, Error::<Runtime>::ProofNotExpectedFromFirstRouter);
					});
				}
			}
		}

		mod three_routers {
			use super::*;

			lazy_static! {
				static ref TEST_DATA: Vec<RouterMessage> = vec![
					(ROUTER_ID_1, Message::Simple),
					(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
					(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
				];
			}

			mod two_messages {
				use super::*;

				const MESSAGE_COUNT: usize = 2;

				#[test]
				fn success() {
					let expected_results: HashMap<Vec<RouterMessage>, ExpectedTestResult> =
						HashMap::from([
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(ROUTER_ID_2, None),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
						]);

					let suite = generate_test_suite(
						vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3],
						TEST_DATA.clone(),
						expected_results,
						MESSAGE_COUNT,
					);

					run_inbound_message_test_suite(suite);
				}
			}

			mod three_messages {
				use super::*;

				const MESSAGE_COUNT: usize = 3;

				#[test]
				fn success() {
					let expected_results: HashMap<Vec<RouterMessage>, ExpectedTestResult> =
						HashMap::from([
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 6,
											}),
										),
										(ROUTER_ID_2, None),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 3,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 3,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											ROUTER_ID_1,
											Some(InboundEntry::<Runtime>::Message {
												session_id: TEST_SESSION_ID,
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(ROUTER_ID_2, None),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(ROUTER_ID_2, None),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(ROUTER_ID_2, None),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(ROUTER_ID_2, None),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(ROUTER_ID_2, None),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(ROUTER_ID_2, None),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_1, Message::Simple),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(ROUTER_ID_2, None),
										(ROUTER_ID_3, None),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(ROUTER_ID_2, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
									(ROUTER_ID_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(ROUTER_ID_1, None),
										(
											ROUTER_ID_2,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 1,
											}),
										),
										(
											ROUTER_ID_3,
											Some(InboundEntry::<Runtime>::Proof {
												session_id: TEST_SESSION_ID,
												current_count: 2,
											}),
										),
									],
								},
							),
						]);

					let suite = generate_test_suite(
						vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3],
						TEST_DATA.clone(),
						expected_results,
						MESSAGE_COUNT,
					);

					run_inbound_message_test_suite(suite);
				}
			}
		}

		#[test]
		fn inbound_message_handler_error() {
			new_test_ext().execute_with(|| {
				let domain_address = DomainAddress::EVM(1, [1; 20]);

				Routers::<Runtime>::set(BoundedVec::try_from(vec![ROUTER_ID_1.clone()]).unwrap());
				SessionIdStore::<Runtime>::set(1);

				let message = Message::Simple;
				let gateway_message = GatewayMessage::Inbound {
					domain_address: domain_address.clone(),
					message: message.clone(),
					router_id: ROUTER_ID_1,
				};

				let err = DispatchError::Unavailable;

				MockLiquidityPools::mock_handle(move |mock_domain_address, mock_mesage| {
					assert_eq!(mock_domain_address, domain_address);
					assert_eq!(mock_mesage, message);

					Err(err)
				});

				let (res, _) = LiquidityPoolsGateway::process(gateway_message);
				assert_noop!(res, err);
			});
		}
	}

	mod outbound {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let sender = TEST_DOMAIN_ADDRESS;
				let message = Message::Simple;

				let gateway_message = GatewayMessage::Outbound {
					sender: sender.clone(),
					message: message.clone(),
					router_id: ROUTER_ID_1,
				};

				let handler = MockMessageSender::mock_send(
					move |mock_router_id, mock_sender, mock_message| {
						assert_eq!(mock_router_id, ROUTER_ID_1);
						assert_eq!(mock_sender, sender);
						assert_eq!(mock_message, message.serialize());

						Ok(())
					},
				);

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_ok!(res);
				assert!(weight.eq(&LP_DEFENSIVE_WEIGHT));
				assert_eq!(handler.times(), 1);
			});
		}

		#[test]
		fn message_sender_error() {
			new_test_ext().execute_with(|| {
				let sender = TEST_DOMAIN_ADDRESS;
				let message = Message::Simple;

				let gateway_message = GatewayMessage::Outbound {
					sender: sender.clone(),
					message: message.clone(),
					router_id: ROUTER_ID_1,
				};

				let router_err = DispatchError::Unavailable;

				MockMessageSender::mock_send(move |mock_router_id, mock_sender, mock_message| {
					assert_eq!(mock_router_id, ROUTER_ID_1);
					assert_eq!(mock_sender, sender);
					assert_eq!(mock_message, message.serialize());

					Err(router_err)
				});

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_noop!(res, router_err);
				assert!(weight.eq(&LP_DEFENSIVE_WEIGHT));
			});
		}
	}
}

mod batches {
	use super::*;

	const USER: AccountId32 = AccountId32::new([1; 32]);
	const OTHER: AccountId32 = AccountId32::new([2; 32]);
	const DOMAIN: Domain = Domain::EVM(TEST_EVM_CHAIN);

	#[test]
	fn pack_empty() {
		new_test_ext().execute_with(|| {
			assert_ok!(LiquidityPoolsGateway::start_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));
			assert_ok!(LiquidityPoolsGateway::end_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));
		});
	}

	#[test]
	fn pack_several() {
		new_test_ext().execute_with(|| {
			assert_ok!(LiquidityPoolsGateway::start_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));

			let handler = MockLiquidityPoolsGatewayQueue::mock_submit(|_| Ok(()));

			// Ok Batched
			assert_ok!(LiquidityPoolsGateway::handle(USER, DOMAIN, Message::Simple));

			Routers::<Runtime>::set(BoundedVec::try_from(vec![ROUTER_ID_1]).unwrap());

			// Not batched, it belongs to OTHER
			assert_ok!(LiquidityPoolsGateway::handle(
				OTHER,
				DOMAIN,
				Message::Simple
			));

			// Not batched, it belongs to EVM 2
			assert_ok!(LiquidityPoolsGateway::handle(
				USER,
				Domain::EVM(2),
				Message::Simple
			));

			// Ok Batched
			assert_ok!(LiquidityPoolsGateway::handle(USER, DOMAIN, Message::Simple));

			// Two non-packed messages
			assert_eq!(handler.times(), 2);

			assert_ok!(LiquidityPoolsGateway::end_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));

			// Packed message queued
			assert_eq!(handler.times(), 3);
		});
	}

	#[test]
	fn pack_over_limit() {
		new_test_ext().execute_with(|| {
			assert_ok!(LiquidityPoolsGateway::start_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));

			let handler = MockLiquidityPoolsGatewayQueue::mock_submit(|_| Ok(()));

			(0..MAX_PACKED_MESSAGES).for_each(|_| {
				assert_ok!(LiquidityPoolsGateway::handle(USER, DOMAIN, Message::Simple));
			});

			assert_err!(
				LiquidityPoolsGateway::handle(USER, DOMAIN, Message::Simple),
				DispatchError::Other(MAX_PACKED_MESSAGES_ERR)
			);

			let router_id_1 = ROUTER_ID_1;

			Routers::<Runtime>::set(BoundedVec::try_from(vec![router_id_1]).unwrap());

			assert_ok!(LiquidityPoolsGateway::end_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));
			assert_eq!(handler.times(), 1);
		});
	}

	#[test]
	fn end_before_start() {
		new_test_ext().execute_with(|| {
			assert_err!(
				LiquidityPoolsGateway::end_batch_message(RuntimeOrigin::signed(USER), DOMAIN),
				Error::<Runtime>::MessagePackingNotStarted
			);
		});
	}

	#[test]
	fn start_before_end() {
		new_test_ext().execute_with(|| {
			assert_ok!(LiquidityPoolsGateway::start_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));

			assert_err!(
				LiquidityPoolsGateway::start_batch_message(RuntimeOrigin::signed(USER), DOMAIN),
				Error::<Runtime>::MessagePackingAlreadyStarted
			);
		});
	}

	#[test]
	fn process_inbound() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(TEST_EVM_CHAIN, address.into());

			let router_id_1 = ROUTER_ID_1;

			Routers::<Runtime>::set(BoundedVec::try_from(vec![router_id_1]).unwrap());
			SessionIdStore::<Runtime>::set(1);

			let handler = MockLiquidityPools::mock_handle(|_, _| Ok(()));

			let submessage_count = 5;

			let (result, weight) = LiquidityPoolsGateway::process(GatewayMessage::Inbound {
				domain_address,
				message: Message::deserialize(&(1..=submessage_count).collect::<Vec<_>>()).unwrap(),
				router_id: ROUTER_ID_1,
			});

			let expected_weight = Weight::default()
				// get_inbound_processing_info
				.saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads(3))
				// process_inbound_message
				.saturating_add(Weight::from_parts(0, Message::max_encoded_len() as u64))
				.saturating_add(LP_DEFENSIVE_WEIGHT)
				// upsert_pending_entry
				.saturating_add(
					<Runtime as frame_system::Config>::DbWeight::get()
						.writes(1)
						.saturating_mul(submessage_count.into()),
				)
				// get_executable_message
				.saturating_add(
					<Runtime as frame_system::Config>::DbWeight::get()
						.reads(1)
						.saturating_mul(submessage_count.into()),
				)
				// decrease_pending_entries_counts
				.saturating_add(
					<Runtime as frame_system::Config>::DbWeight::get()
						.writes(1)
						.saturating_mul(submessage_count.into()),
				)
				// process_inbound_message
				.saturating_mul(submessage_count.into());

			assert_ok!(result);
			assert_eq!(weight, expected_weight);
			assert_eq!(handler.times(), submessage_count as u32);
		});
	}

	#[test]
	fn process_inbound_with_errors() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(1, address.into());

			let router_id_1 = ROUTER_ID_1;

			Routers::<Runtime>::set(BoundedVec::try_from(vec![router_id_1]).unwrap());
			SessionIdStore::<Runtime>::set(1);

			let counter = Arc::new(AtomicU32::new(0));

			let handler = MockLiquidityPools::mock_handle(move |_, _| {
				match counter.fetch_add(1, Ordering::Relaxed) {
					2 => Err(DispatchError::Unavailable),
					_ => Ok(()),
				}
			});

			let (result, _) = LiquidityPoolsGateway::process(GatewayMessage::Inbound {
				domain_address,
				message: Message::deserialize(&(1..=5).collect::<Vec<_>>()).unwrap(),
				router_id: ROUTER_ID_1,
			});

			assert_err!(result, DispatchError::Unavailable);
			// 2 correct messages and 1 failed message processed.
			assert_eq!(handler.times(), 3);
		});
	}
}

mod execute_message_recovery {
	use super::*;

	#[test]
	fn success_with_execution() {
		new_test_ext().execute_with(|| {
			let session_id = 1;

			Routers::<Runtime>::set(BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2]).unwrap());
			SessionIdStore::<Runtime>::set(session_id);

			PendingInboundEntries::<Runtime>::insert(
				MESSAGE_PROOF,
				ROUTER_ID_1,
				InboundEntry::<Runtime>::Message {
					session_id,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: Message::Simple,
					expected_proof_count: 1,
				},
			);

			let handler =
				MockLiquidityPools::mock_handle(move |mock_domain_address, mock_message| {
					assert_eq!(mock_domain_address, TEST_DOMAIN_ADDRESS);
					assert_eq!(mock_message, Message::Simple);

					Ok(())
				});

			assert_ok!(LiquidityPoolsGateway::execute_message_recovery(
				RuntimeOrigin::root(),
				TEST_DOMAIN_ADDRESS,
				MESSAGE_PROOF,
				ROUTER_ID_2,
			));

			event_exists(Event::<Runtime>::MessageRecoveryExecuted {
				proof: MESSAGE_PROOF,
				router_id: ROUTER_ID_2,
			});

			assert_eq!(handler.times(), 1);

			assert!(PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_1).is_none());
			assert!(PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_2).is_none());
		});
	}

	#[test]
	fn success_without_execution() {
		new_test_ext().execute_with(|| {
			let session_id = 1;

			Routers::<Runtime>::set(
				BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3]).unwrap(),
			);
			SessionIdStore::<Runtime>::set(session_id);

			PendingInboundEntries::<Runtime>::insert(
				MESSAGE_PROOF,
				ROUTER_ID_1,
				InboundEntry::<Runtime>::Message {
					session_id,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: Message::Simple,
					expected_proof_count: 2,
				},
			);

			assert_ok!(LiquidityPoolsGateway::execute_message_recovery(
				RuntimeOrigin::root(),
				TEST_DOMAIN_ADDRESS,
				MESSAGE_PROOF,
				ROUTER_ID_2,
			));

			event_exists(Event::<Runtime>::MessageRecoveryExecuted {
				proof: MESSAGE_PROOF,
				router_id: ROUTER_ID_2,
			});

			assert_eq!(
				PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_1),
				Some(InboundEntry::<Runtime>::Message {
					session_id,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: Message::Simple,
					expected_proof_count: 2,
				})
			);
			assert_eq!(
				PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_2),
				Some(InboundEntry::<Runtime>::Proof {
					session_id,
					current_count: 1
				})
			);
			assert!(PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_3).is_none())
		});
	}

	#[test]
	fn not_enough_routers_for_domain() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				LiquidityPoolsGateway::execute_message_recovery(
					RuntimeOrigin::root(),
					TEST_DOMAIN_ADDRESS,
					MESSAGE_PROOF,
					ROUTER_ID_1,
				),
				Error::<Runtime>::NotEnoughRoutersForDomain
			);

			Routers::<Runtime>::set(BoundedVec::try_from(vec![ROUTER_ID_1]).unwrap());

			assert_noop!(
				LiquidityPoolsGateway::execute_message_recovery(
					RuntimeOrigin::root(),
					TEST_DOMAIN_ADDRESS,
					MESSAGE_PROOF,
					ROUTER_ID_1,
				),
				Error::<Runtime>::NotEnoughRoutersForDomain
			);
		});
	}

	#[test]
	fn unknown_router() {
		new_test_ext().execute_with(|| {
			Routers::<Runtime>::set(BoundedVec::try_from(vec![ROUTER_ID_1]).unwrap());
			SessionIdStore::<Runtime>::set(1);

			assert_noop!(
				LiquidityPoolsGateway::execute_message_recovery(
					RuntimeOrigin::root(),
					TEST_DOMAIN_ADDRESS,
					MESSAGE_PROOF,
					ROUTER_ID_2
				),
				Error::<Runtime>::UnknownRouter
			);
		});
	}

	#[test]
	fn proof_count_overflow() {
		new_test_ext().execute_with(|| {
			let session_id = 1;

			Routers::<Runtime>::set(BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2]).unwrap());
			SessionIdStore::<Runtime>::set(session_id);
			PendingInboundEntries::<Runtime>::insert(
				MESSAGE_PROOF,
				ROUTER_ID_2,
				InboundEntry::<Runtime>::Proof {
					session_id,
					current_count: u32::MAX,
				},
			);

			assert_noop!(
				LiquidityPoolsGateway::execute_message_recovery(
					RuntimeOrigin::root(),
					TEST_DOMAIN_ADDRESS,
					MESSAGE_PROOF,
					ROUTER_ID_2
				),
				Arithmetic(Overflow)
			);
		});
	}

	#[test]
	fn expected_message_proof_type() {
		new_test_ext().execute_with(|| {
			let domain_address = TEST_DOMAIN_ADDRESS;
			let session_id = 1;

			Routers::<Runtime>::set(BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2]).unwrap());
			SessionIdStore::<Runtime>::set(session_id);
			PendingInboundEntries::<Runtime>::insert(
				MESSAGE_PROOF,
				ROUTER_ID_2,
				InboundEntry::<Runtime>::Message {
					session_id,
					domain_address: domain_address.clone(),
					message: Message::Simple,
					expected_proof_count: 2,
				},
			);

			assert_noop!(
				LiquidityPoolsGateway::execute_message_recovery(
					RuntimeOrigin::root(),
					TEST_DOMAIN_ADDRESS,
					MESSAGE_PROOF,
					ROUTER_ID_2
				),
				Error::<Runtime>::ExpectedMessageProofType
			);
		});
	}
}
