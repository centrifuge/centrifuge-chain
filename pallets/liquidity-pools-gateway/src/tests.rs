use std::collections::HashMap;

use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{LPEncoding, MessageProcessor, OutboundMessageHandler};
use cfg_types::domain_address::*;
use frame_support::{
	assert_err, assert_noop, assert_ok, dispatch::PostDispatchInfo, pallet_prelude::Pays,
	weights::Weight,
};
use itertools::Itertools;
use lazy_static::lazy_static;
use parity_scale_codec::MaxEncodedLen;
use sp_core::{bounded::BoundedVec, crypto::AccountId32, ByteArray, H160, H256};
use sp_runtime::{DispatchError, DispatchError::BadOrigin};
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

pub const TEST_DOMAIN_ADDRESS: DomainAddress = DomainAddress::EVM(0, [1; 20]);

lazy_static! {
	static ref ROUTER_HASH_1: H256 = H256::from_low_u64_be(1);
	static ref ROUTER_HASH_2: H256 = H256::from_low_u64_be(2);
	static ref ROUTER_HASH_3: H256 = H256::from_low_u64_be(3);
}

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

mod set_domain_routers {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);

			let router_id_1 = H256::from_low_u64_be(1);
			let router_id_2 = H256::from_low_u64_be(2);
			let router_id_3 = H256::from_low_u64_be(3);

			//TODO(cdamian): Enable this after we figure out router init?
			// let router = RouterMock::<Runtime>::default();
			// router.mock_init(move || Ok(()));

			let router_ids =
				BoundedVec::try_from(vec![router_id_1, router_id_2, router_id_3]).unwrap();

			assert_ok!(LiquidityPoolsGateway::set_domain_routers(
				RuntimeOrigin::root(),
				domain.clone(),
				router_ids.clone(),
			));

			assert_eq!(Routers::<Runtime>::get(domain.clone()).unwrap(), router_ids);
			assert_eq!(
				InboundMessageSessions::<Runtime>::get(domain.clone()),
				Some(1)
			);
			assert_eq!(InvalidSessionIds::<Runtime>::get(0), Some(()));

			event_exists(Event::<Runtime>::RoutersSet { domain, router_ids });
		});
	}

	//TODO(cdamian): Enable this after we figure out router init?
	//
	// fn router_init_error() {
	// 	new_test_ext().execute_with(|| {
	// 		let domain = Domain::EVM(0);
	// 		let router = RouterMock::<Runtime>::default();
	// 		router.mock_init(move || Err(DispatchError::Other("error")));
	//
	// 		assert_noop!(
	// 			LiquidityPoolsGateway::set_domain_router(
	// 				RuntimeOrigin::root(),
	// 				domain.clone(),
	// 				router,
	// 			),
	// 			Error::<Runtime>::RouterInitFailed,
	// 		);
	// 	});
	// }
	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);

			assert_noop!(
				LiquidityPoolsGateway::set_domain_routers(
					RuntimeOrigin::signed(get_test_account_id()),
					domain.clone(),
					BoundedVec::try_from(vec![]).unwrap(),
				),
				BadOrigin
			);

			assert!(Routers::<Runtime>::get(domain.clone()).is_none());
			assert!(InboundMessageSessions::<Runtime>::get(domain).is_none());
			assert!(InvalidSessionIds::<Runtime>::get(0).is_none());
		});
	}

	#[test]
	fn unsupported_domain() {
		new_test_ext().execute_with(|| {
			let domain = Domain::Centrifuge;

			assert_noop!(
				LiquidityPoolsGateway::set_domain_routers(
					RuntimeOrigin::root(),
					domain.clone(),
					BoundedVec::try_from(vec![]).unwrap(),
				),
				Error::<Runtime>::DomainNotSupported
			);

			assert!(Routers::<Runtime>::get(domain.clone()).is_none());
			assert!(InboundMessageSessions::<Runtime>::get(domain).is_none());
			assert!(InvalidSessionIds::<Runtime>::get(0).is_none());
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

			let router_id = H256::from_low_u64_be(1);

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let encoded_msg = message.serialize();

			let gateway_message = GatewayMessage::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
				router_id,
			};

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_message| {
				assert_eq!(mock_message, gateway_message);
				Ok(())
			});

			assert_ok!(LiquidityPoolsGateway::receive_message(
				GatewayOrigin::Domain(domain_address).into(),
				router_id,
				BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
			));
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let encoded_msg = Message::Simple.serialize();

			let router_id = H256::from_low_u64_be(1);

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
			let router_id = H256::from_low_u64_be(1);

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
			let router_id = H256::from_low_u64_be(1);

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

			let router_id = H256::from_low_u64_be(1);

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let encoded_msg = message.serialize();

			let err = sp_runtime::DispatchError::from("liquidity_pools error");

			let gateway_message = GatewayMessage::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
				router_id,
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

			let router_id_1 = H256::from_low_u64_be(1);
			let router_id_2 = H256::from_low_u64_be(2);
			let router_id_3 = H256::from_low_u64_be(3);

			//TODO(cdamian): Router init
			// let router_hash_1 = H256::from_low_u64_be(1);
			// let router_hash_2 = H256::from_low_u64_be(2);
			// let router_hash_3 = H256::from_low_u64_be(3);
			//
			// let router_mock_1 = RouterMock::<Runtime>::default();
			// let router_mock_2 = RouterMock::<Runtime>::default();
			// let router_mock_3 = RouterMock::<Runtime>::default();
			//
			// router_mock_1.mock_init(move || Ok(()));
			// router_mock_1.mock_hash(move || router_hash_1);
			// router_mock_2.mock_init(move || Ok(()));
			// router_mock_2.mock_hash(move || router_hash_2);
			// router_mock_3.mock_init(move || Ok(()));
			// router_mock_3.mock_hash(move || router_hash_3);

			assert_ok!(LiquidityPoolsGateway::set_domain_routers(
				RuntimeOrigin::root(),
				domain.clone(),
				BoundedVec::try_from(vec![router_id_1, router_id_2, router_id_3]).unwrap(),
			));

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_msg| {
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
		});
	}

	#[test]
	fn local_domain() {
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
	fn message_queue_error() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let sender = get_test_account_id();
			let msg = Message::Simple;

			let router_id_1 = H256::from_low_u64_be(1);
			let router_id_2 = H256::from_low_u64_be(2);
			let router_id_3 = H256::from_low_u64_be(3);

			//TODO(cdamian): Router init?
			// let router_hash_1 = H256::from_low_u64_be(1);
			// let router_hash_2 = H256::from_low_u64_be(2);
			// let router_hash_3 = H256::from_low_u64_be(3);
			//
			// let router_mock_1 = RouterMock::<Runtime>::default();
			// let router_mock_2 = RouterMock::<Runtime>::default();
			// let router_mock_3 = RouterMock::<Runtime>::default();
			//
			// router_mock_1.mock_init(move || Ok(()));
			// router_mock_1.mock_hash(move || router_hash_1);
			// router_mock_2.mock_init(move || Ok(()));
			// router_mock_2.mock_hash(move || router_hash_2);
			// router_mock_3.mock_init(move || Ok(()));
			// router_mock_3.mock_hash(move || router_hash_3);

			assert_ok!(LiquidityPoolsGateway::set_domain_routers(
				RuntimeOrigin::root(),
				domain.clone(),
				BoundedVec::try_from(vec![router_id_1, router_id_2, router_id_3]).unwrap(),
			));

			let gateway_message = GatewayMessage::Outbound {
				sender: <Runtime as Config>::Sender::get(),
				message: msg.clone(),
				router_id: router_id_1,
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
	fn failure_bad_origin() {
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
	fn failure_centrifuge_domain() {
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
						let session_id = 1;

						Routers::<Runtime>::insert(
							TEST_DOMAIN_ADDRESS.domain(),
							BoundedVec::try_from(test_routers.clone()).unwrap(),
						);
						InboundMessageSessions::<Runtime>::insert(
							TEST_DOMAIN_ADDRESS.domain(),
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
							let expected_storage_entry_router_hash = expected_storage_entry.0;
							let expected_inbound_entry = expected_storage_entry.1;

							let storage_entry = PendingInboundEntries::<Runtime>::get(
								session_id,
								(MESSAGE_PROOF, expected_storage_entry_router_hash),
							);
							assert_eq!(storage_entry, expected_inbound_entry, "Expected inbound entry {expected_inbound_entry:?}, found {storage_entry:?}");
						}
					});
				}
			}

			/// Used for generating all `RouterMessage` combinations like:
			///
			/// vec![
			/// 	(*ROUTER_HASH_1, Message::Simple),
			/// 	(*ROUTER_HASH_1, Message::Simple),
			/// ]
			/// vec![
			/// 	(*ROUTER_HASH_1, Message::Simple),
			/// 	(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
			/// ]
			/// vec![
			/// 	(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
			/// 	(*ROUTER_HASH_1, Message::Simple),
			/// ]
			/// vec![
			/// 	(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
			/// 	(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
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
			pub type RouterMessage = (H256, Message);

			/// Type used for aggregating tests for inbound messages.
			pub struct InboundMessageTestSuite {
				pub routers: Vec<H256>,
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
				pub expected_storage_entries: Vec<(H256, Option<InboundEntry<Runtime>>)>,
			}

			/// Generates the combinations of `RouterMessage` used when testing,
			/// maps the `ExpectedTestResult` for each and creates the
			/// `InboundMessageTestSuite`.
			pub fn generate_test_suite(
				routers: Vec<H256>,
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
					let router_hash = *ROUTER_HASH_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						router_id: router_hash,
					};

					Routers::<Runtime>::insert(
						domain_address.domain(),
						BoundedVec::<_, _>::try_from(vec![router_hash]).unwrap(),
					);
					InboundMessageSessions::<Runtime>::insert(domain_address.domain(), session_id);

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

					assert!(PendingInboundEntries::<Runtime>::get(
						session_id,
						(message_proof, router_hash)
					)
					.is_none());
				});
			}

			#[test]
			fn multi_router_not_found() {
				new_test_ext().execute_with(|| {
					let message = Message::Simple;
					let domain_address = DomainAddress::EVM(1, [1; 20]);
					let router_hash = *ROUTER_HASH_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						router_id: router_hash,
					};

					let (res, _) = LiquidityPoolsGateway::process(gateway_message);
					assert_noop!(res, Error::<Runtime>::MultiRouterNotFound);
				});
			}

			#[test]
			fn inbound_domain_session_not_found() {
				new_test_ext().execute_with(|| {
					let message = Message::Simple;
					let domain_address = DomainAddress::EVM(1, [1; 20]);
					let router_hash = *ROUTER_HASH_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						router_id: router_hash,
					};

					Routers::<Runtime>::insert(
						domain_address.domain(),
						BoundedVec::<_, _>::try_from(vec![router_hash]).unwrap(),
					);

					let (res, _) = LiquidityPoolsGateway::process(gateway_message);
					assert_noop!(res, Error::<Runtime>::InboundDomainSessionNotFound);
				});
			}

			#[test]
			fn unknown_inbound_message_router() {
				new_test_ext().execute_with(|| {
					let message = Message::Simple;
					let session_id = 1;
					let domain_address = DomainAddress::EVM(1, [1; 20]);
					let router_hash = *ROUTER_HASH_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						// The router stored has a different hash, this should trigger the expected
						// error.
						router_id: *ROUTER_HASH_2,
					};

					Routers::<Runtime>::insert(
						domain_address.domain(),
						BoundedVec::<_, _>::try_from(vec![router_hash]).unwrap(),
					);
					InboundMessageSessions::<Runtime>::insert(domain_address.domain(), session_id);

					let (res, _) = LiquidityPoolsGateway::process(gateway_message);
					assert_noop!(res, Error::<Runtime>::UnknownInboundMessageRouter);
				});
			}

			#[test]
			fn expected_message_proof_type() {
				new_test_ext().execute_with(|| {
					let message = Message::Simple;
					let message_proof = message.to_message_proof().get_message_proof().unwrap();
					let session_id = 1;
					let domain_address = DomainAddress::EVM(1, [1; 20]);
					let router_hash = *ROUTER_HASH_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						router_id: router_hash,
					};

					Routers::<Runtime>::insert(
						domain_address.domain(),
						BoundedVec::<_, _>::try_from(vec![router_hash]).unwrap(),
					);
					InboundMessageSessions::<Runtime>::insert(domain_address.domain(), session_id);
					PendingInboundEntries::<Runtime>::insert(
						session_id,
						(message_proof, router_hash),
						InboundEntry::<Runtime>::Proof { current_count: 0 },
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
						(*ROUTER_HASH_1, Message::Simple),
						(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
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
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 2,
												}),
											),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(*ROUTER_HASH_2, None),
										],
									},
								),
							]);

						let suite = generate_test_suite(
							vec![*ROUTER_HASH_1, *ROUTER_HASH_2],
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
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 3,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 3,
												}),
											),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 1,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 1,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 1,
												}),
											),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 1,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 1,
												}),
											),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 1,
												}),
											),
										],
									},
								),
							]);

						let suite = generate_test_suite(
							vec![*ROUTER_HASH_1, *ROUTER_HASH_2],
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
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 4,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 0,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 4,
												}),
											),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(
												*ROUTER_HASH_1,
												Some(InboundEntry::<Runtime>::Message {
													domain_address: TEST_DOMAIN_ADDRESS,
													message: Message::Simple,
													expected_proof_count: 2,
												}),
											),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 2,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(*ROUTER_HASH_2, None),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 2,
												}),
											),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 2,
												}),
											),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 2,
												}),
											),
										],
									},
								),
								(
									vec![
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
										(*ROUTER_HASH_1, Message::Simple),
									],
									ExpectedTestResult {
										message_submitted_times: 1,
										expected_storage_entries: vec![
											(*ROUTER_HASH_1, None),
											(
												*ROUTER_HASH_2,
												Some(InboundEntry::<Runtime>::Proof {
													current_count: 2,
												}),
											),
										],
									},
								),
							]);

						let suite = generate_test_suite(
							vec![*ROUTER_HASH_1, *ROUTER_HASH_2],
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

						Routers::<Runtime>::insert(
							TEST_DOMAIN_ADDRESS.domain(),
							BoundedVec::<_, _>::try_from(vec![*ROUTER_HASH_1, *ROUTER_HASH_2])
								.unwrap(),
						);
						InboundMessageSessions::<Runtime>::insert(
							TEST_DOMAIN_ADDRESS.domain(),
							session_id,
						);

						let gateway_message = GatewayMessage::Inbound {
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Simple,
							router_id: *ROUTER_HASH_2,
						};

						let (res, _) = LiquidityPoolsGateway::process(gateway_message);
						assert_noop!(res, Error::<Runtime>::MessageExpectedFromFirstRouter);
					});
				}

				#[test]
				fn proof_not_expected_from_first_router() {
					new_test_ext().execute_with(|| {
						let session_id = 1;

						Routers::<Runtime>::insert(
							TEST_DOMAIN_ADDRESS.domain(),
							BoundedVec::<_, _>::try_from(vec![*ROUTER_HASH_1, *ROUTER_HASH_2])
								.unwrap(),
						);
						InboundMessageSessions::<Runtime>::insert(
							TEST_DOMAIN_ADDRESS.domain(),
							session_id,
						);

						let gateway_message = GatewayMessage::Inbound {
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Proof(MESSAGE_PROOF),
							router_id: *ROUTER_HASH_1,
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
					(*ROUTER_HASH_1, Message::Simple),
					(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
					(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
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
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(*ROUTER_HASH_2, None),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
						]);

					let suite = generate_test_suite(
						vec![*ROUTER_HASH_1, *ROUTER_HASH_2, *ROUTER_HASH_3],
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
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 6,
											}),
										),
										(*ROUTER_HASH_2, None),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 3,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 3,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 2,
											}),
										),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(
											*ROUTER_HASH_1,
											Some(InboundEntry::<Runtime>::Message {
												domain_address: TEST_DOMAIN_ADDRESS,
												message: Message::Simple,
												expected_proof_count: 4,
											}),
										),
										(*ROUTER_HASH_2, None),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(*ROUTER_HASH_2, None),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(*ROUTER_HASH_2, None),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(*ROUTER_HASH_2, None),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(*ROUTER_HASH_2, None),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(*ROUTER_HASH_2, None),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_1, Message::Simple),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 1,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(*ROUTER_HASH_2, None),
										(*ROUTER_HASH_3, None),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
									],
								},
							),
							(
								vec![
									(*ROUTER_HASH_2, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
									(*ROUTER_HASH_3, Message::Proof(MESSAGE_PROOF)),
								],
								ExpectedTestResult {
									message_submitted_times: 0,
									expected_storage_entries: vec![
										(*ROUTER_HASH_1, None),
										(
											*ROUTER_HASH_2,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 1,
											}),
										),
										(
											*ROUTER_HASH_3,
											Some(InboundEntry::<Runtime>::Proof {
												current_count: 2,
											}),
										),
									],
								},
							),
						]);

					let suite = generate_test_suite(
						vec![*ROUTER_HASH_1, *ROUTER_HASH_2, *ROUTER_HASH_3],
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

				let router_id = H256::from_low_u64_be(1);

				Routers::<Runtime>::insert(
					domain_address.domain(),
					BoundedVec::try_from(vec![router_id]).unwrap(),
				);
				InboundMessageSessions::<Runtime>::insert(domain_address.domain(), 1);

				let message = Message::Simple;
				let gateway_message = GatewayMessage::Inbound {
					domain_address: domain_address.clone(),
					message: message.clone(),
					router_id,
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
				let sender = get_test_account_id();
				let domain = Domain::EVM(1);
				let message = Message::Simple;

				let expected_sender = sender.clone();
				let expected_message = message.clone();

				let router_post_info = PostDispatchInfo {
					actual_weight: Some(Weight::from_parts(1, 1)),
					pays_fee: Pays::Yes,
				};

				let router_id = H256::from_low_u64_be(1);

				//TODO(cdamian): Drop mock?
				// let router_hash = H256::from_low_u64_be(1);
				//
				// let router_mock = RouterMock::<Runtime>::default();
				// router_mock.mock_send(move |mock_sender, mock_message| {
				// 	assert_eq!(mock_sender, expected_sender);
				// 	assert_eq!(mock_message, expected_message.serialize());
				//
				// 	Ok(router_post_info)
				// });
				// router_mock.mock_hash(move || router_hash);
				//
				// DomainRouters::<Runtime>::insert(domain.clone(), router_mock);

				let min_expected_weight = <Runtime as frame_system::Config>::DbWeight::get()
					.reads(1) + router_post_info.actual_weight.unwrap()
					+ Weight::from_parts(0, message.serialize().len() as u64);

				let gateway_message = GatewayMessage::Outbound {
					sender,
					message: message.clone(),
					router_id,
				};

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_ok!(res);
				assert!(weight.all_lte(min_expected_weight));
			});
		}

		//TODO(cdamian): Fix when bi-directional routers are in.
		// #[test]
		// fn router_not_found() {
		// 	new_test_ext().execute_with(|| {
		// 		let sender = get_test_account_id();
		// 		let message = Message::Simple;
		//
		// 		let expected_weight = <Runtime as
		// frame_system::Config>::DbWeight::get().reads(1);
		//
		// 		let gateway_message = GatewayMessage::Outbound {
		// 			sender,
		// 			message,
		// 			router_id: H256::from_low_u64_be(1),
		// 		};
		//
		// 		let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
		// 		assert_noop!(res, Error::<Runtime>::RouterNotFound);
		// 		assert_eq!(weight, expected_weight);
		// 	});
		// }

		#[test]
		fn router_error() {
			new_test_ext().execute_with(|| {
				let sender = get_test_account_id();
				let domain = Domain::EVM(1);
				let message = Message::Simple;

				let expected_sender = sender.clone();
				let expected_message = message.clone();

				let router_post_info = PostDispatchInfo {
					actual_weight: Some(Weight::from_parts(1, 1)),
					pays_fee: Pays::Yes,
				};

				// let router_err = DispatchError::Unavailable;
				//
				// let router_mock = RouterMock::<Runtime>::default();
				// router_mock.mock_send(move |mock_sender, mock_message| {
				// 	assert_eq!(mock_sender, expected_sender);
				// 	assert_eq!(mock_message, expected_message.serialize());
				//
				// 	Err(DispatchErrorWithPostInfo {
				// 		post_info: router_post_info,
				// 		error: router_err,
				// 	})
				// });
				//
				// DomainRouters::<Runtime>::insert(domain.clone(), router_mock);

				let min_expected_weight = <Runtime as frame_system::Config>::DbWeight::get()
					.reads(1) + router_post_info.actual_weight.unwrap()
					+ Weight::from_parts(0, message.serialize().len() as u64);

				let gateway_message = GatewayMessage::Outbound {
					sender,
					message: message.clone(),
					router_id: H256::from_low_u64_be(1),
				};

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				//TODO(cdamian): Error out
				assert_ok!(res);
				// assert_noop!(res, router_err)
				assert!(weight.all_lte(min_expected_weight));
			});
		}
	}
}

mod batches {
	use super::*;

	const USER: AccountId32 = AccountId32::new([1; 32]);
	const OTHER: AccountId32 = AccountId32::new([2; 32]);
	const DOMAIN: Domain = Domain::EVM(1);

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

			let handle = MockLiquidityPoolsGatewayQueue::mock_submit(|_| Ok(()));

			// Ok Batched
			assert_ok!(LiquidityPoolsGateway::handle(USER, DOMAIN, Message::Simple));

			let router_id_1 = H256::from_low_u64_be(1);

			Routers::<Runtime>::insert(DOMAIN, BoundedVec::try_from(vec![router_id_1]).unwrap());

			// Not batched, it belong to OTHER
			assert_ok!(LiquidityPoolsGateway::handle(
				OTHER,
				DOMAIN,
				Message::Simple
			));

			Routers::<Runtime>::insert(
				Domain::EVM(2),
				BoundedVec::try_from(vec![router_id_1]).unwrap(),
			);

			// Not batched, it belong to EVM 2
			assert_ok!(LiquidityPoolsGateway::handle(
				USER,
				Domain::EVM(2),
				Message::Simple
			));

			// Ok Batched
			assert_ok!(LiquidityPoolsGateway::handle(USER, DOMAIN, Message::Simple));

			// Just the two non-packed messages
			assert_eq!(handle.times(), 2);

			assert_ok!(LiquidityPoolsGateway::end_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));

			// Packed message queued
			assert_eq!(handle.times(), 3);
		});
	}

	#[test]
	fn pack_over_limit() {
		new_test_ext().execute_with(|| {
			assert_ok!(LiquidityPoolsGateway::start_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));

			MockLiquidityPoolsGatewayQueue::mock_submit(|_| Ok(()));

			(0..MAX_PACKED_MESSAGES).for_each(|_| {
				assert_ok!(LiquidityPoolsGateway::handle(USER, DOMAIN, Message::Simple));
			});

			assert_err!(
				LiquidityPoolsGateway::handle(USER, DOMAIN, Message::Simple),
				DispatchError::Other(MAX_PACKED_MESSAGES_ERR)
			);

			let router_id_1 = H256::from_low_u64_be(1);

			Routers::<Runtime>::insert(DOMAIN, BoundedVec::try_from(vec![router_id_1]).unwrap());

			assert_ok!(LiquidityPoolsGateway::end_batch_message(
				RuntimeOrigin::signed(USER),
				DOMAIN
			));
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
			let domain_address = DomainAddress::EVM(0, address.into());

			let router_id_1 = H256::from_low_u64_be(1);

			Routers::<Runtime>::insert(
				domain_address.domain(),
				BoundedVec::try_from(vec![router_id_1]).unwrap(),
			);
			InboundMessageSessions::<Runtime>::insert(domain_address.domain(), 1);

			let handler = MockLiquidityPools::mock_handle(|_, _| Ok(()));

			let submessage_count = 5;

			let (result, weight) = LiquidityPoolsGateway::process(GatewayMessage::Inbound {
				domain_address,
				message: Message::deserialize(&(1..=submessage_count).collect::<Vec<_>>()).unwrap(),
				router_id: *ROUTER_HASH_1,
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
			let domain_address = DomainAddress::EVM(0, address.into());

			let router_id_1 = H256::from_low_u64_be(1);

			Routers::<Runtime>::insert(
				domain_address.domain(),
				BoundedVec::try_from(vec![router_id_1]).unwrap(),
			);
			InboundMessageSessions::<Runtime>::insert(domain_address.domain(), 1);

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
				router_id: *ROUTER_HASH_1,
			});

			assert_err!(result, DispatchError::Unavailable);
			// 2 correct messages and 1 failed message processed.
			assert_eq!(handler.times(), 3);
		});
	}
}
