use std::collections::HashMap;

use cfg_mocks::*;
use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{LPEncoding, MessageProcessor, OutboundMessageHandler, Proof};
use cfg_types::domain_address::*;
use frame_support::{
	assert_err, assert_noop, assert_ok, dispatch::PostDispatchInfo, pallet_prelude::Pays,
	weights::Weight,
};
use itertools::Itertools;
use lazy_static::lazy_static;
use parity_scale_codec::MaxEncodedLen;
use sp_core::{bounded::BoundedVec, crypto::AccountId32, ByteArray, H160, H256};
use sp_runtime::{DispatchError, DispatchError::BadOrigin, DispatchErrorWithPostInfo};
use sp_std::sync::{
	atomic::{AtomicU32, Ordering},
	Arc,
};

use super::{
	mock::{RuntimeEvent as MockEvent, *},
	origin::*,
	pallet::*,
};
use crate::{GatewayMessage, InboundEntry};

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

mod set_domain_router {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = RouterMock::<Runtime>::default();
			router.mock_init(move || Ok(()));

			assert_ok!(LiquidityPoolsGateway::set_domain_router(
				RuntimeOrigin::root(),
				domain.clone(),
				router.clone(),
			));

			let storage_entry = DomainRouters::<Runtime>::get(domain.clone());
			assert_eq!(storage_entry.unwrap(), router);

			event_exists(Event::<Runtime>::DomainRouterSet { domain, router });
		});
	}
	#[test]
	fn router_init_error() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = RouterMock::<Runtime>::default();
			router.mock_init(move || Err(DispatchError::Other("error")));

			assert_noop!(
				LiquidityPoolsGateway::set_domain_router(
					RuntimeOrigin::root(),
					domain.clone(),
					router,
				),
				Error::<Runtime>::RouterInitFailed,
			);
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = RouterMock::<Runtime>::default();

			assert_noop!(
				LiquidityPoolsGateway::set_domain_router(
					RuntimeOrigin::signed(get_test_account_id()),
					domain.clone(),
					router,
				),
				BadOrigin
			);

			let storage_entry = DomainRouters::<Runtime>::get(domain);
			assert!(storage_entry.is_none());
		});
	}

	#[test]
	fn unsupported_domain() {
		new_test_ext().execute_with(|| {
			let domain = Domain::Centrifuge;
			let router = RouterMock::<Runtime>::default();

			assert_noop!(
				LiquidityPoolsGateway::set_domain_router(
					RuntimeOrigin::root(),
					domain.clone(),
					router
				),
				Error::<Runtime>::DomainNotSupported
			);

			let storage_entry = DomainRouters::<Runtime>::get(domain);
			assert!(storage_entry.is_none());
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

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let encoded_msg = message.serialize();

			let gateway_message = GatewayMessage::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
				router_hash: H256::from_low_u64_be(1),
			};

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_message| {
				assert_eq!(mock_message, gateway_message);
				Ok(())
			});

			assert_ok!(LiquidityPoolsGateway::receive_message(
				GatewayOrigin::Domain(domain_address).into(),
				BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
			));
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let encoded_msg = Message::Simple.serialize();

			assert_noop!(
				LiquidityPoolsGateway::receive_message(
					RuntimeOrigin::signed(AccountId32::new([0u8; 32])),
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

			assert_noop!(
				LiquidityPoolsGateway::receive_message(
					GatewayOrigin::Domain(domain_address).into(),
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

			assert_noop!(
				LiquidityPoolsGateway::receive_message(
					GatewayOrigin::Domain(domain_address).into(),
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

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let encoded_msg = message.serialize();

			let err = sp_runtime::DispatchError::from("liquidity_pools error");

			let gateway_message = GatewayMessage::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
				router_hash: H256::from_low_u64_be(1),
			};

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_message| {
				assert_eq!(mock_message, gateway_message);
				Err(err)
			});

			assert_noop!(
				LiquidityPoolsGateway::receive_message(
					GatewayOrigin::Domain(domain_address).into(),
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

			let router_hash_1 = H256::from_low_u64_be(1);
			let router_hash_2 = H256::from_low_u64_be(2);
			let router_hash_3 = H256::from_low_u64_be(3);

			let router_mock_1 = RouterMock::<Runtime>::default();
			let router_mock_2 = RouterMock::<Runtime>::default();
			let router_mock_3 = RouterMock::<Runtime>::default();

			router_mock_1.mock_init(move || Ok(()));
			router_mock_1.mock_hash(move || router_hash_1);
			router_mock_2.mock_init(move || Ok(()));
			router_mock_2.mock_hash(move || router_hash_2);
			router_mock_3.mock_init(move || Ok(()));
			router_mock_3.mock_hash(move || router_hash_3);

			assert_ok!(LiquidityPoolsGateway::set_outbound_routers(
				RuntimeOrigin::root(),
				domain.clone(),
				BoundedVec::try_from(vec![router_mock_1, router_mock_2, router_mock_3]).unwrap(),
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

			let router_hash_1 = H256::from_low_u64_be(1);
			let router_hash_2 = H256::from_low_u64_be(2);
			let router_hash_3 = H256::from_low_u64_be(3);

			let router_mock_1 = RouterMock::<Runtime>::default();
			let router_mock_2 = RouterMock::<Runtime>::default();
			let router_mock_3 = RouterMock::<Runtime>::default();

			router_mock_1.mock_init(move || Ok(()));
			router_mock_1.mock_hash(move || router_hash_1);
			router_mock_2.mock_init(move || Ok(()));
			router_mock_2.mock_hash(move || router_hash_2);
			router_mock_3.mock_init(move || Ok(()));
			router_mock_3.mock_hash(move || router_hash_3);

			assert_ok!(LiquidityPoolsGateway::set_outbound_routers(
				RuntimeOrigin::root(),
				domain.clone(),
				BoundedVec::try_from(vec![router_mock_1, router_mock_2, router_mock_3]).unwrap(),
			));

			let gateway_message = GatewayMessage::Outbound {
				sender: <Runtime as Config>::Sender::get(),
				message: msg.clone(),
				router_hash: router_hash_3,
			};

			let err = DispatchError::Unavailable;

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_msg| {
				assert_eq!(mock_msg, gateway_message);

				Err(err)
			});

			assert_noop!(LiquidityPoolsGateway::handle(sender, domain, msg), err);
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

			macro_rules! run_tests {
				($tests:expr) => {
					// $tests = Vec<(Vec<Message>, &ExpectedTestResult)>
					for test in $tests {
						new_test_ext().execute_with(|| {
							println!("Executing test for - {:?}", test.0);

							let handler = MockLiquidityPools::mock_handle(move |_, _| Ok(()));

							// test.0 = Vec<Message>
							for test_message in test.0 {
								let domain_address = DomainAddress::EVM(1, [1; 20]);
								let gateway_message = GatewayMessage::Inbound {
									domain_address: domain_address.clone(),
									message: test_message.clone(),
									//TODO(cdamian): Use test router hash.
									router_hash: H256::from_low_u64_be(1),
								};

								let (res, _) = LiquidityPoolsGateway::process(gateway_message);
								assert_ok!(res);
							}

							assert_eq!(handler.times(), test.1.mock_called_times);

							assert_eq!(
								InboundMessages::<Runtime>::get(MESSAGE_PROOF),
								// test.1 = &ExpectedTestResult
								test.1.inbound_message,
							);
							assert_eq!(
								InboundMessageProofCount::<Runtime>::get(MESSAGE_PROOF),
								// test.1 = &ExpectedTestResult
								test.1.proof_count,
							);
						});
					}
				};
			}

			lazy_static! {
				static ref TEST_MESSAGES: Vec<Message> =
					vec![Message::Simple, Message::Proof(MESSAGE_PROOF),];
			}

			/// Generate all `Message` combinations for a specific
			/// number of messages, like:
			///
			/// vec![
			///		Message::Simple,
			/// 	Message::Simple,
			/// ]
			/// vec![
			/// 	Message::Simple,
			/// 	Message::Proof(MESSAGE_PROOF),
			/// ]
			/// vec![
			///     Message::Proof(MESSAGE_PROOF),
			/// 	Message::Simple,
			/// ]
			/// vec![
			/// 	Message::Proof(MESSAGE_PROOF),
			/// 	Message::Proof(MESSAGE_PROOF),
			/// ]
			pub fn generate_test_combinations(count: usize) -> Vec<Vec<Message>> {
				std::iter::repeat(TEST_MESSAGES.clone().into_iter())
					.take(count)
					.multi_cartesian_product()
					.collect::<Vec<_>>()
			}

			pub struct ExpectedTestResult {
				pub inbound_message: Option<(DomainAddress, Message, u32)>,
				pub proof_count: u32,
				pub mock_called_times: u32,
			}

			pub fn gen_new<T>(t: T, count: usize) -> Vec<Vec<<T as IntoIterator>::Item>>
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
		}

		use util::*;

		mod one_router {
			use super::*;

			#[test]
			fn success() {
				new_test_ext().execute_with(|| {
					let message = Message::Simple;
					let session_id = 1;
					let domain_address = DomainAddress::EVM(1, [1; 20]);
					let router_hash = *ROUTER_HASH_1;
					let gateway_message = GatewayMessage::Inbound {
						domain_address: domain_address.clone(),
						message: message.clone(),
						router_hash,
					};

					InboundRouters::<Runtime>::insert(
						domain_address.domain(),
						BoundedVec::<_, _>::try_from(vec![router_hash]).unwrap(),
					);
					InboundDomainSessions::<Runtime>::insert(domain_address.domain(), session_id);

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
						router_hash,
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
						router_hash,
					};

					InboundRouters::<Runtime>::insert(
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
						router_hash: *ROUTER_HASH_2,
					};

					InboundRouters::<Runtime>::insert(
						domain_address.domain(),
						BoundedVec::<_, _>::try_from(vec![router_hash]).unwrap(),
					);
					InboundDomainSessions::<Runtime>::insert(domain_address.domain(), session_id);

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
						router_hash,
					};

					InboundRouters::<Runtime>::insert(
						domain_address.domain(),
						BoundedVec::<_, _>::try_from(vec![router_hash]).unwrap(),
					);
					InboundDomainSessions::<Runtime>::insert(domain_address.domain(), session_id);
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

		// mod combined_messages {
		// 	use super::*;
		//
		// 	mod two_messages {
		// 		use super::*;
		//
		// 		lazy_static! {
		// 			static ref TEST_MAP: HashMap<Vec<Message>, ExpectedTestResult> =
		// 				HashMap::from([
		// 					(
		// 						vec![Message::Simple, Message::Simple],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								4
		// 							)),
		// 							proof_count: 0,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![Message::Proof(MESSAGE_PROOF), Message::Proof(MESSAGE_PROOF)],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 2,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![Message::Simple, Message::Proof(MESSAGE_PROOF)],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								2
		// 							)),
		// 							proof_count: 1,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![Message::Proof(MESSAGE_PROOF), Message::Simple],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								2
		// 							)),
		// 							proof_count: 1,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 				]);
		// 		}
		//
		// 		#[test]
		// 		fn two_messages() {
		// 			let tests = generate_test_combinations(2)
		// 				.iter()
		// 				.map(|x| {
		// 					(
		// 						x.clone(),
		// 						TEST_MAP
		// 							.get(x)
		// 							.expect(format!("test for {x:?} should be covered").as_str()),
		// 					)
		// 				})
		// 				.collect::<Vec<_>>();
		//
		// 			run_tests!(tests);
		// 		}
		// 	}
		//
		// 	mod three_messages {
		// 		use super::*;
		//
		// 		lazy_static! {
		// 			static ref TEST_MAP: HashMap<Vec<Message>, ExpectedTestResult> =
		// 				HashMap::from([
		// 					(
		// 						vec![Message::Simple, Message::Simple, Message::Simple,],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								6
		// 							)),
		// 							proof_count: 0,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 3,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 0,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 0,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 0,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								4
		// 							)),
		// 							proof_count: 1,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								4
		// 							)),
		// 							proof_count: 1,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								4
		// 							)),
		// 							proof_count: 1,
		// 							mock_called_times: 0,
		// 						}
		// 					)
		// 				]);
		// 		}
		//
		// 		#[test]
		// 		fn three_messages() {
		// 			let tests = generate_test_combinations(3)
		// 				.iter()
		// 				.map(|x| {
		// 					(
		// 						x.clone(),
		// 						TEST_MAP
		// 							.get(x)
		// 							.expect(format!("test for {x:?} should be covered").as_str()),
		// 					)
		// 				})
		// 				.collect::<Vec<_>>();
		//
		// 			run_tests!(tests);
		// 		}
		// 	}
		//
		// 	mod four_messages {
		// 		use super::*;
		//
		// 		lazy_static! {
		// 			static ref TEST_MAP: HashMap<Vec<Message>, ExpectedTestResult> =
		// 				HashMap::from([
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Simple,
		// 							Message::Simple,
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								8
		// 							)),
		// 							proof_count: 0,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 4,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 1,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 1,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 1,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: None,
		// 							proof_count: 1,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								2
		// 							)),
		// 							proof_count: 0,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								2
		// 							)),
		// 							proof_count: 0,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								2
		// 							)),
		// 							proof_count: 0,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								2
		// 							)),
		// 							proof_count: 0,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								2
		// 							)),
		// 							proof_count: 0,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								2
		// 							)),
		// 							proof_count: 0,
		// 							mock_called_times: 1,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Simple,
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								6
		// 							)),
		// 							proof_count: 1,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								6
		// 							)),
		// 							proof_count: 1,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Simple,
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								6
		// 							)),
		// 							proof_count: 1,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 					(
		// 						vec![
		// 							Message::Proof(MESSAGE_PROOF),
		// 							Message::Simple,
		// 							Message::Simple,
		// 							Message::Simple,
		// 						],
		// 						ExpectedTestResult {
		// 							inbound_message: Some((
		// 								DomainAddress::EVM(1, [1; 20]),
		// 								Message::Simple,
		// 								6
		// 							)),
		// 							proof_count: 1,
		// 							mock_called_times: 0,
		// 						}
		// 					),
		// 				]);
		// 		}
		//
		// 		#[test]
		// 		fn four_messages() {
		// 			let tests = generate_test_combinations(4)
		// 				.iter()
		// 				.filter(|x| TEST_MAP.get(x.clone()).is_some())
		// 				.map(|x| {
		// 					(
		// 						x.clone(),
		// 						TEST_MAP
		// 							.get(x)
		// 							.expect(format!("test for {x:?} should be covered").as_str()),
		// 					)
		// 				})
		// 				.collect::<Vec<_>>();
		//
		// 			run_tests!(tests);
		// 		}
		// 	}
		// }
		//
		// #[test]
		// fn two_non_proof_and_four_proofs() {
		// 	let tests = generate_test_combinations(6)
		// 		.into_iter()
		// 		.filter(|x| {
		// 			let r = x.iter().counts_by(|c| c.clone());
		// 			let non_proof_count = r.get(&Message::Simple);
		// 			let proof_count = r.get(&Message::Proof(MESSAGE_PROOF));
		//
		// 			match (non_proof_count, proof_count) {
		// 				(Some(non_proof_count), Some(proof_count)) => {
		// 					*non_proof_count == 2 && *proof_count == 4
		// 				}
		// 				_ => false,
		// 			}
		// 		})
		// 		.map(|x| {
		// 			(
		// 				x,
		// 				ExpectedTestResult {
		// 					inbound_message: None,
		// 					proof_count: 0,
		// 					mock_called_times: 2,
		// 				},
		// 			)
		// 		})
		// 		.collect::<Vec<_>>();
		//
		// 	run_tests!(tests);
		// }

		#[test]
		fn inbound_message_handler_error() {
			new_test_ext().execute_with(|| {
				let domain_address = DomainAddress::EVM(1, [1; 20]);

				let message = Message::Proof(MESSAGE_PROOF);
				let gateway_message = GatewayMessage::Inbound {
					domain_address: domain_address.clone(),
					message: message.clone(),
					router_hash: H256::from_low_u64_be(1),
				};

				let (res, _) = LiquidityPoolsGateway::process(gateway_message);
				assert_ok!(res);

				let message = Message::Proof(MESSAGE_PROOF);
				let gateway_message = GatewayMessage::Inbound {
					domain_address: domain_address.clone(),
					message: message.clone(),
					router_hash: H256::from_low_u64_be(1),
				};

				let (res, _) = LiquidityPoolsGateway::process(gateway_message);
				assert_ok!(res);

				let message = Message::Simple;
				let gateway_message = GatewayMessage::Inbound {
					domain_address: domain_address.clone(),
					message: message.clone(),
					router_hash: H256::from_low_u64_be(1),
				};

				let err = DispatchError::Unavailable;

				MockLiquidityPools::mock_handle(move |mock_domain_address, mock_mesage| {
					assert_eq!(mock_domain_address, domain_address);
					assert_eq!(mock_mesage, message);

					Err(err)
				});

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_noop!(res, err);
				assert_eq!(weight, LP_DEFENSIVE_WEIGHT);
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

				let router_hash = H256::from_low_u64_be(1);

				let router_mock = RouterMock::<Runtime>::default();
				router_mock.mock_send(move |mock_sender, mock_message| {
					assert_eq!(mock_sender, expected_sender);
					assert_eq!(mock_message, expected_message.serialize());

					Ok(router_post_info)
				});
				router_mock.mock_hash(move || router_hash);

				DomainRouters::<Runtime>::insert(domain.clone(), router_mock);

				let min_expected_weight = <Runtime as frame_system::Config>::DbWeight::get()
					.reads(1) + router_post_info.actual_weight.unwrap()
					+ Weight::from_parts(0, message.serialize().len() as u64);

				let gateway_message = GatewayMessage::Outbound {
					sender,
					message: message.clone(),
					router_hash,
				};

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_ok!(res);
				assert!(weight.all_lte(min_expected_weight));
			});
		}

		#[test]
		fn router_not_found() {
			new_test_ext().execute_with(|| {
				let sender = get_test_account_id();
				let message = Message::Simple;

				let expected_weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				let gateway_message = GatewayMessage::Outbound {
					sender,
					message,
					router_hash: H256::from_low_u64_be(1),
				};

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_noop!(res, Error::<Runtime>::RouterNotFound);
				assert_eq!(weight, expected_weight);
			});
		}

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

				let router_err = DispatchError::Unavailable;

				let router_mock = RouterMock::<Runtime>::default();
				router_mock.mock_send(move |mock_sender, mock_message| {
					assert_eq!(mock_sender, expected_sender);
					assert_eq!(mock_message, expected_message.serialize());

					Err(DispatchErrorWithPostInfo {
						post_info: router_post_info,
						error: router_err,
					})
				});

				DomainRouters::<Runtime>::insert(domain.clone(), router_mock);

				let min_expected_weight = <Runtime as frame_system::Config>::DbWeight::get()
					.reads(1) + router_post_info.actual_weight.unwrap()
					+ Weight::from_parts(0, message.serialize().len() as u64);

				let gateway_message = GatewayMessage::Outbound {
					sender,
					message: message.clone(),
					router_hash: H256::from_low_u64_be(1),
				};

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_noop!(res, router_err);
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

			// Not batched, it belong to OTHER
			assert_ok!(LiquidityPoolsGateway::handle(
				OTHER,
				DOMAIN,
				Message::Simple
			));

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

			MockLiquidityPools::mock_handle(|_, _| Ok(()));

			let (result, weight) = LiquidityPoolsGateway::process(GatewayMessage::Inbound {
				domain_address,
				message: Message::deserialize(&(1..=5).collect::<Vec<_>>()).unwrap(),
				router_hash: *ROUTER_HASH_1,
			});

			assert_eq!(weight, LP_DEFENSIVE_WEIGHT * 5);
			assert_ok!(result);
		});
	}

	#[test]
	fn process_inbound_with_errors() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			let counter = Arc::new(AtomicU32::new(0));
			MockLiquidityPools::mock_handle(move |_, _| {
				match counter.fetch_add(1, Ordering::Relaxed) {
					2 => Err(DispatchError::Unavailable),
					_ => Ok(()),
				}
			});

			let (result, weight) = LiquidityPoolsGateway::process(GatewayMessage::Inbound {
				domain_address,
				message: Message::deserialize(&(1..=5).collect::<Vec<_>>()).unwrap(),
				router_hash: *ROUTER_HASH_1,
			});

			// 2 correct messages and 1 failed message processed.
			assert_eq!(weight, LP_DEFENSIVE_WEIGHT * 3);
			assert_err!(result, DispatchError::Unavailable);
		});
	}
}
