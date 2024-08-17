use std::collections::HashMap;

use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{LpMessage, MessageProcessor, OutboundMessageHandler};
use cfg_types::domain_address::*;
use frame_support::{assert_err, assert_noop, assert_ok};
use itertools::Itertools;
use lazy_static::lazy_static;
use sp_arithmetic::ArithmeticError::{Overflow, Underflow};
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
use crate::{
	message_processing::{InboundEntry, MessageEntry, ProofEntry},
	GatewayMessage,
};

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

mod extrinsics {
	use super::*;

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

				router_ids =
					BoundedVec::try_from(vec![ROUTER_ID_3, ROUTER_ID_2, ROUTER_ID_1]).unwrap();

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
				let domain_address = DomainAddress::Evm(0, address);

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
				let domain_address = DomainAddress::Evm(0, address);

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
					LiquidityPoolsGateway::add_instance(
						RuntimeOrigin::root(),
						domain_address.clone()
					),
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
				let domain_address = DomainAddress::Evm(0, address);

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
				let domain_address = DomainAddress::Evm(0, address);

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
				let domain_address = DomainAddress::Evm(0, address);

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
				let domain_address = DomainAddress::Evm(0, address);

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

	mod receive_message {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
				let domain_address = DomainAddress::Evm(0, address);
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
				let domain_address = DomainAddress::Evm(0, address);
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
				let domain_address = DomainAddress::Evm(0, address);
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

	mod set_domain_hook {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let domain = Domain::Evm(0);

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
				let domain = Domain::Evm(0);

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

	mod batches {
		use super::*;

		const USER: AccountId32 = AccountId32::new([1; 32]);
		const OTHER: AccountId32 = AccountId32::new([2; 32]);
		const DOMAIN: Domain = Domain::Evm(TEST_EVM_CHAIN);

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
					Domain::Evm(2),
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
				let domain_address = DomainAddress::Evm(TEST_EVM_CHAIN, address);

				let router_id_1 = ROUTER_ID_1;

				Routers::<Runtime>::set(BoundedVec::try_from(vec![router_id_1]).unwrap());
				SessionIdStore::<Runtime>::set(1);

				let handler = MockLiquidityPools::mock_handle(|_, _| Ok(()));

				let submessage_count = 5;

				let (result, weight) = LiquidityPoolsGateway::process(GatewayMessage::Inbound {
					domain_address,
					message: Message::deserialize(&(1..=submessage_count).collect::<Vec<_>>())
						.unwrap(),
					router_id: ROUTER_ID_1,
				});

				let expected_weight = LP_DEFENSIVE_WEIGHT.saturating_mul(submessage_count.into());

				assert_ok!(result);
				assert_eq!(weight, expected_weight);
				assert_eq!(handler.times(), submessage_count as u32);
			});
		}

		#[test]
		fn process_inbound_with_errors() {
			new_test_ext().execute_with(|| {
				let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
				let domain_address = DomainAddress::Evm(1, address);

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

				Routers::<Runtime>::set(
					BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2]).unwrap(),
				);
				SessionIdStore::<Runtime>::set(session_id);

				PendingInboundEntries::<Runtime>::insert(
					MESSAGE_PROOF,
					ROUTER_ID_1,
					InboundEntry::Message(MessageEntry {
						session_id,
						domain_address: TEST_DOMAIN_ADDRESS,
						message: Message::Simple,
						expected_proof_count: 1,
					}),
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

				assert!(
					PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_1).is_none()
				);
				assert!(
					PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_2).is_none()
				);
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
					InboundEntry::Message(MessageEntry {
						session_id,
						domain_address: TEST_DOMAIN_ADDRESS,
						message: Message::Simple,
						expected_proof_count: 2,
					}),
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
					Some(
						MessageEntry {
							session_id,
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Simple,
							expected_proof_count: 2,
						}
						.into()
					)
				);
				assert_eq!(
					PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_2),
					Some(
						ProofEntry {
							session_id,
							current_count: 1
						}
						.into()
					)
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

				Routers::<Runtime>::set(
					BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2]).unwrap(),
				);
				SessionIdStore::<Runtime>::set(session_id);
				PendingInboundEntries::<Runtime>::insert(
					MESSAGE_PROOF,
					ROUTER_ID_2,
					InboundEntry::Proof(ProofEntry {
						session_id,
						current_count: u32::MAX,
					}),
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

				Routers::<Runtime>::set(
					BoundedVec::try_from(vec![ROUTER_ID_1, ROUTER_ID_2]).unwrap(),
				);
				SessionIdStore::<Runtime>::set(session_id);
				PendingInboundEntries::<Runtime>::insert(
					MESSAGE_PROOF,
					ROUTER_ID_2,
					InboundEntry::Message(MessageEntry {
						session_id,
						domain_address: domain_address.clone(),
						message: Message::Simple,
						expected_proof_count: 2,
					}),
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
}

mod implementations {
	use super::*;

	mod outbound_message_handler {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let domain = Domain::Evm(0);
				let sender = get_test_account_id();
				let msg = Message::Simple;
				let message_proof = msg.to_proof_message().get_proof().unwrap();

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
				let domain = Domain::Evm(0);
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
				let domain = Domain::Evm(0);
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

	mod message_processor {
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

				/// Type used for defining the number of expected inbound
				/// message submission and the exected storage state.
				#[derive(Clone, Debug)]
				pub struct ExpectedTestResult {
					pub message_submitted_times: u32,
					pub expected_storage_entries: Vec<(RouterId, Option<InboundEntry<Runtime>>)>,
				}

				/// Generates the combinations of `RouterMessage` used when
				/// testing, maps the `ExpectedTestResult` for each and
				/// creates the `InboundMessageTestSuite`.
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
									format!("test for {router_messages:?} should be covered")
										.as_str(),
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
						let message_proof = message.to_proof_message().get_proof().unwrap();
						let session_id = 1;
						let domain_address = DomainAddress::Evm(1, H160::repeat_byte(1));
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
							PendingInboundEntries::<Runtime>::get(message_proof, router_id)
								.is_none()
						);
					});
				}

				#[test]
				fn multi_router_not_found() {
					new_test_ext().execute_with(|| {
						let message = Message::Simple;
						let domain_address = DomainAddress::Evm(1, H160::repeat_byte(1));
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
						let domain_address = DomainAddress::Evm(1, H160::repeat_byte(1));
						let router_hash = ROUTER_ID_1;
						let gateway_message = GatewayMessage::Inbound {
							domain_address: domain_address.clone(),
							message: message.clone(),
							// The router stored has a different hash, this should trigger the
							// expected error.
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
						let message_proof = message.to_proof_message().get_proof().unwrap();
						let session_id = 1;
						let domain_address = DomainAddress::Evm(1, H160::repeat_byte(1));
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
							InboundEntry::Proof(ProofEntry {
								session_id,
								current_count: 0,
							}),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 2,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 2,
														}
														.into(),
													),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 3,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 3,
														}
														.into(),
													),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 1,
														}
														.into(),
													),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 1,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 1,
														}
														.into(),
													),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 1,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 1,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 1,
														}
														.into(),
													),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 4,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 4,
														}
														.into(),
													),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 2,
														}
														.into(),
													),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 2,
														}
														.into(),
													),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 2,
														}
														.into(),
													),
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
													Some(
														MessageEntry {
															session_id: TEST_SESSION_ID,
															domain_address: TEST_DOMAIN_ADDRESS,
															message: Message::Simple,
															expected_proof_count: 2,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 2,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 2,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 2,
														}
														.into(),
													),
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
													Some(
														ProofEntry {
															session_id: TEST_SESSION_ID,
															current_count: 2,
														}
														.into(),
													),
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
								BoundedVec::<_, _>::try_from(vec![ROUTER_ID_1, ROUTER_ID_2])
									.unwrap(),
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
								BoundedVec::<_, _>::try_from(vec![ROUTER_ID_1, ROUTER_ID_2])
									.unwrap(),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 4,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_2,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(ROUTER_ID_2, None),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(ROUTER_ID_2, None),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_2,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(ROUTER_ID_2, None),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 6,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 3,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 3,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 4,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_2,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 4,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_2,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_2,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(ROUTER_ID_2, None),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(ROUTER_ID_2, None),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(ROUTER_ID_2, None),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_2,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 2,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_2,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 4,
													}
													.into(),
												),
											),
											(ROUTER_ID_2, None),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 4,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_2,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 4,
													}
													.into(),
												),
											),
											(ROUTER_ID_2, None),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													MessageEntry {
														session_id: TEST_SESSION_ID,
														domain_address: TEST_DOMAIN_ADDRESS,
														message: Message::Simple,
														expected_proof_count: 4,
													}
													.into(),
												),
											),
											(ROUTER_ID_2, None),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
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
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 1,
													}
													.into(),
												),
											),
											(
												ROUTER_ID_3,
												Some(
													ProofEntry {
														session_id: TEST_SESSION_ID,
														current_count: 2,
													}
													.into(),
												),
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
					let domain_address = DomainAddress::Evm(1, H160::repeat_byte(1));

					Routers::<Runtime>::set(
						BoundedVec::try_from(vec![ROUTER_ID_1.clone()]).unwrap(),
					);
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
							assert_eq!(mock_message, message);

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

					MockMessageSender::mock_send(
						move |mock_router_id, mock_sender, mock_message| {
							assert_eq!(mock_router_id, ROUTER_ID_1);
							assert_eq!(mock_sender, sender);
							assert_eq!(mock_message, message);

							Err(router_err)
						},
					);

					let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
					assert_noop!(res, router_err);
					assert!(weight.eq(&LP_DEFENSIVE_WEIGHT));
				});
			}
		}
	}

	mod pallet {
		use super::*;

		mod get_router_ids_for_domain {
			use super::*;

			#[test]
			fn success() {
				new_test_ext().execute_with(|| {
					let domain = TEST_DOMAIN_ADDRESS.domain();
					let test_routers = vec![ROUTER_ID_1];

					Routers::<Runtime>::set(BoundedVec::try_from(test_routers.clone()).unwrap());

					let res = LiquidityPoolsGateway::get_router_ids_for_domain(domain).unwrap();
					assert_eq!(res, test_routers);
				});
			}

			#[test]
			fn not_enough_routers_for_domain() {
				new_test_ext().execute_with(|| {
					let domain = TEST_DOMAIN_ADDRESS.domain();

					let res = LiquidityPoolsGateway::get_router_ids_for_domain(domain.clone());

					assert_eq!(
						res.err().unwrap(),
						Error::<Runtime>::NotEnoughRoutersForDomain.into()
					);

					let test_routers = vec![RouterId(4)];

					Routers::<Runtime>::set(BoundedVec::try_from(test_routers.clone()).unwrap());

					let res = LiquidityPoolsGateway::get_router_ids_for_domain(domain);

					assert_eq!(
						res.err().unwrap(),
						Error::<Runtime>::NotEnoughRoutersForDomain.into()
					);
				});
			}
		}

		mod get_expected_proof_count {
			use super::*;

			#[test]
			fn success() {
				new_test_ext().execute_with(|| {
					let tests = vec![
						vec![ROUTER_ID_1],
						vec![ROUTER_ID_1, ROUTER_ID_2],
						vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3],
					];

					for test in tests {
						let res = LiquidityPoolsGateway::get_expected_proof_count(&test).unwrap();

						assert_eq!(res, (test.len() - 1) as u32);
					}
				});
			}

			#[test]
			fn not_enough_routers_for_domain() {
				new_test_ext().execute_with(|| {
					let res = LiquidityPoolsGateway::get_expected_proof_count(&vec![]);

					assert_eq!(
						res.err().unwrap(),
						Error::<Runtime>::NotEnoughRoutersForDomain.into()
					);
				});
			}
		}

		mod create_inbound_entry {
			use super::*;

			#[test]
			fn create_inbound_entry() {
				new_test_ext().execute_with(|| {
					let domain_address = TEST_DOMAIN_ADDRESS;
					let session_id = 1;
					let expected_proof_count = 2;

					let tests: Vec<(Message, InboundEntry<Runtime>)> = vec![
						(
							Message::Simple,
							MessageEntry {
								session_id,
								domain_address: domain_address.clone(),
								message: Message::Simple,
								expected_proof_count,
							}
							.into(),
						),
						(
							Message::Proof(MESSAGE_PROOF),
							ProofEntry {
								session_id,
								current_count: 1,
							}
							.into(),
						),
					];

					for (test_message, expected_inbound_entry) in tests {
						let res = InboundEntry::create(
							test_message,
							session_id,
							domain_address.clone(),
							expected_proof_count,
						);

						assert_eq!(res, expected_inbound_entry)
					}
				});
			}
		}

		mod upsert_pending_entry {
			use super::*;

			#[test]
			fn no_stored_entry() {
				new_test_ext().execute_with(|| {
					let domain_address = TEST_DOMAIN_ADDRESS;
					let session_id = 1;
					let expected_proof_count = 2;

					let tests: Vec<(RouterId, InboundEntry<Runtime>)> = vec![
						(
							ROUTER_ID_1,
							MessageEntry {
								session_id,
								domain_address,
								message: Message::Simple,
								expected_proof_count,
							}
							.into(),
						),
						(
							ROUTER_ID_2,
							ProofEntry {
								session_id,
								current_count: 1,
							}
							.into(),
						),
					];

					for (test_router_id, test_inbound_entry) in tests {
						assert_ok!(LiquidityPoolsGateway::upsert_pending_entry(
							MESSAGE_PROOF,
							&test_router_id.clone(),
							test_inbound_entry.clone(),
						));

						let res =
							PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, test_router_id)
								.unwrap();

						assert_eq!(res, test_inbound_entry);
					}
				});
			}

			#[test]
			fn message_entry_same_session() {
				new_test_ext().execute_with(|| {
					let inbound_entry: InboundEntry<Runtime> = MessageEntry {
						session_id: 1,
						domain_address: TEST_DOMAIN_ADDRESS,
						message: Message::Simple,
						expected_proof_count: 2,
					}
					.into();

					PendingInboundEntries::<Runtime>::insert(
						MESSAGE_PROOF,
						ROUTER_ID_1,
						inbound_entry.clone(),
					);

					assert_ok!(LiquidityPoolsGateway::upsert_pending_entry(
						MESSAGE_PROOF,
						&ROUTER_ID_1,
						inbound_entry,
					));

					let res =
						PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_1).unwrap();
					assert_eq!(
						res,
						MessageEntry {
							session_id: 1,
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Simple,
							expected_proof_count: 4,
						}
						.into()
					);
				});
			}

			#[test]
			fn message_entry_new_session() {
				new_test_ext().execute_with(|| {
					PendingInboundEntries::<Runtime>::insert(
						MESSAGE_PROOF,
						ROUTER_ID_1,
						InboundEntry::Message(MessageEntry {
							session_id: 1,
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Simple,
							expected_proof_count: 2,
						}),
					);

					assert_ok!(LiquidityPoolsGateway::upsert_pending_entry(
						MESSAGE_PROOF,
						&ROUTER_ID_1,
						MessageEntry {
							session_id: 2,
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Simple,
							expected_proof_count: 2,
						}
						.into(),
					));

					let res =
						PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_1).unwrap();
					assert_eq!(
						res,
						MessageEntry {
							session_id: 2,
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Simple,
							expected_proof_count: 2,
						}
						.into()
					);
				});
			}

			#[test]
			fn expected_message_type() {
				new_test_ext().execute_with(|| {
					let inbound_entry: InboundEntry<Runtime> = MessageEntry {
						session_id: 1,
						domain_address: TEST_DOMAIN_ADDRESS,
						message: Message::Simple,
						expected_proof_count: 2,
					}
					.into();

					PendingInboundEntries::<Runtime>::insert(
						MESSAGE_PROOF,
						ROUTER_ID_1,
						inbound_entry.clone(),
					);

					assert_noop!(
						LiquidityPoolsGateway::upsert_pending_entry(
							MESSAGE_PROOF,
							&ROUTER_ID_1,
							InboundEntry::Proof(ProofEntry {
								session_id: 1,
								current_count: 1
							}),
						),
						Error::<Runtime>::ExpectedMessageType
					);
				});
			}

			#[test]
			fn proof_entry_same_session() {
				new_test_ext().execute_with(|| {
					let inbound_entry: InboundEntry<Runtime> = ProofEntry {
						session_id: 1,
						current_count: 1,
					}
					.into();

					PendingInboundEntries::<Runtime>::insert(
						MESSAGE_PROOF,
						ROUTER_ID_1,
						inbound_entry.clone(),
					);

					assert_ok!(LiquidityPoolsGateway::upsert_pending_entry(
						MESSAGE_PROOF,
						&ROUTER_ID_1,
						inbound_entry,
					));

					let res =
						PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_1).unwrap();
					assert_eq!(
						res,
						ProofEntry {
							session_id: 1,
							current_count: 2,
						}
						.into()
					);
				});
			}

			#[test]
			fn proof_entry_new_session() {
				new_test_ext().execute_with(|| {
					PendingInboundEntries::<Runtime>::insert(
						MESSAGE_PROOF,
						ROUTER_ID_1,
						InboundEntry::Proof(ProofEntry {
							session_id: 1,
							current_count: 2,
						}),
					);

					assert_ok!(LiquidityPoolsGateway::upsert_pending_entry(
						MESSAGE_PROOF,
						&ROUTER_ID_1,
						ProofEntry {
							session_id: 2,
							current_count: 1,
						}
						.into(),
					));

					let res =
						PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_1).unwrap();
					assert_eq!(
						res,
						ProofEntry {
							session_id: 2,
							current_count: 1,
						}
						.into()
					);
				});
			}

			#[test]
			fn expected_message_proof_type() {
				new_test_ext().execute_with(|| {
					let inbound_entry: InboundEntry<Runtime> = ProofEntry {
						session_id: 1,
						current_count: 1,
					}
					.into();

					PendingInboundEntries::<Runtime>::insert(
						MESSAGE_PROOF,
						ROUTER_ID_1,
						inbound_entry.clone(),
					);

					assert_noop!(
						LiquidityPoolsGateway::upsert_pending_entry(
							MESSAGE_PROOF,
							&ROUTER_ID_1,
							InboundEntry::Message(MessageEntry {
								session_id: 1,
								domain_address: TEST_DOMAIN_ADDRESS,
								message: Message::Simple,
								expected_proof_count: 1,
							}),
						),
						Error::<Runtime>::ExpectedMessageProofType
					);
				});
			}
		}

		mod execute_if_requirements_are_met {
			use super::*;

			#[test]
			fn entries_with_invalid_session_are_ignored() {
				new_test_ext().execute_with(|| {
					let domain_address = TEST_DOMAIN_ADDRESS;
					let router_ids = vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3];
					let session_id = 1;
					let expected_proof_count = 2;

					PendingInboundEntries::<Runtime>::insert(
						MESSAGE_PROOF,
						ROUTER_ID_1,
						InboundEntry::Message(MessageEntry {
							session_id: 1,
							domain_address: TEST_DOMAIN_ADDRESS,
							message: Message::Simple,
							expected_proof_count: 2,
						}),
					);
					PendingInboundEntries::<Runtime>::insert(
						MESSAGE_PROOF,
						ROUTER_ID_2,
						InboundEntry::Proof(ProofEntry {
							session_id: 2,
							current_count: 1,
						}),
					);
					PendingInboundEntries::<Runtime>::insert(
						MESSAGE_PROOF,
						ROUTER_ID_3,
						InboundEntry::Proof(ProofEntry {
							session_id: 3,
							current_count: 1,
						}),
					);

					assert_ok!(LiquidityPoolsGateway::execute_if_requirements_are_met(
						MESSAGE_PROOF,
						&router_ids,
						session_id,
						expected_proof_count,
						domain_address,
					));
					assert!(
						PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_1).is_some()
					);
					assert!(
						PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_2).is_some()
					);
					assert!(
						PendingInboundEntries::<Runtime>::get(MESSAGE_PROOF, ROUTER_ID_3).is_some()
					);
				});
			}
		}

		mod execute_post_voting_dispatch {
			use super::*;

			#[test]
			fn pending_inbound_entry_not_found() {
				new_test_ext().execute_with(|| {
					let router_ids = vec![ROUTER_ID_1];
					let expected_proof_count = 2;

					assert_noop!(
						LiquidityPoolsGateway::execute_post_voting_dispatch(
							MESSAGE_PROOF,
							&router_ids,
							expected_proof_count,
						),
						Error::<Runtime>::PendingInboundEntryNotFound
					);
				});
			}
		}
	}
}

mod inbound_entry {
	use super::*;

	mod create_post_voting_entry {
		use super::*;

		#[test]
		fn message_entry_some() {
			new_test_ext().execute_with(|| {
				let message = Message::Simple;

				let inbound_entry = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id: 1,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: message.clone(),
					expected_proof_count: 4,
				});

				let expected_proof_count = 2;

				let res =
					InboundEntry::create_post_voting_entry(&inbound_entry, expected_proof_count)
						.unwrap();

				assert_eq!(
					res,
					Some(InboundEntry::<Runtime>::Message(MessageEntry {
						session_id: 1,
						domain_address: TEST_DOMAIN_ADDRESS,
						message,
						expected_proof_count: 2,
					}))
				);
			});
		}

		#[test]
		fn message_entry_count_underflow() {
			new_test_ext().execute_with(|| {
				let message = Message::Simple;

				let inbound_entry = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id: 1,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: message.clone(),
					expected_proof_count: 2,
				});

				let expected_proof_count = 3;

				let res =
					InboundEntry::create_post_voting_entry(&inbound_entry, expected_proof_count);

				assert_noop!(res, Arithmetic(Underflow));
			});
		}

		#[test]
		fn message_entry_zero_updated_count() {
			new_test_ext().execute_with(|| {
				let message = Message::Simple;

				let inbound_entry = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id: 1,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: message.clone(),
					expected_proof_count: 2,
				});

				let expected_proof_count = 2;

				let res =
					InboundEntry::create_post_voting_entry(&inbound_entry, expected_proof_count)
						.unwrap();

				assert_eq!(res, None);
			});
		}

		#[test]
		fn proof_entry_some() {
			new_test_ext().execute_with(|| {
				let inbound_entry = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id: 1,
					current_count: 2,
				});

				let expected_proof_count = 2;

				let res =
					InboundEntry::create_post_voting_entry(&inbound_entry, expected_proof_count)
						.unwrap();

				assert_eq!(
					res,
					Some(InboundEntry::<Runtime>::Proof(ProofEntry {
						session_id: 1,
						current_count: 1
					}))
				);
			});
		}

		#[test]
		fn proof_entry_count_underflow() {
			new_test_ext().execute_with(|| {
				let inbound_entry = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id: 1,
					current_count: 0,
				});

				let expected_proof_count = 2;

				let res =
					InboundEntry::create_post_voting_entry(&inbound_entry, expected_proof_count);

				assert_noop!(res, Arithmetic(Underflow));
			});
		}

		#[test]
		fn proof_entry_zero_updated_count() {
			new_test_ext().execute_with(|| {
				let inbound_entry = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id: 1,
					current_count: 1,
				});

				let expected_proof_count = 2;

				let res =
					InboundEntry::create_post_voting_entry(&inbound_entry, expected_proof_count)
						.unwrap();

				assert_eq!(res, None,);
			});
		}
	}

	mod validate {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let domain_address = TEST_DOMAIN_ADDRESS;
				let router_ids = vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3];
				let session_id = 1;
				let expected_proof_count = 2;

				let inbound_entry = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id,
					domain_address,
					message: Message::Simple,
					expected_proof_count,
				});

				assert_ok!(inbound_entry.validate(&router_ids, &ROUTER_ID_1));

				let inbound_entry = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id,
					current_count: 1,
				});

				assert_ok!(inbound_entry.validate(&router_ids, &ROUTER_ID_2));
			});
		}

		#[test]
		fn unknown_router() {
			new_test_ext().execute_with(|| {
				let router_ids = vec![ROUTER_ID_1, ROUTER_ID_2];
				let session_id = 1;

				let inbound_entry = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id,
					current_count: 1,
				});

				assert_noop!(
					inbound_entry.validate(&router_ids, &ROUTER_ID_3),
					Error::<Runtime>::UnknownRouter
				);
			});
		}

		#[test]
		fn message_type_mismatch() {
			new_test_ext().execute_with(|| {
				let domain_address = TEST_DOMAIN_ADDRESS;
				let router_ids = vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3];
				let session_id = 1;
				let expected_proof_count = 2;

				let inbound_entry = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id,
					domain_address,
					message: Message::Simple,
					expected_proof_count,
				});

				assert_noop!(
					inbound_entry.validate(&router_ids, &ROUTER_ID_2),
					Error::<Runtime>::MessageExpectedFromFirstRouter
				);

				let inbound_entry = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id,
					current_count: 1,
				});

				assert_noop!(
					inbound_entry.validate(&router_ids, &ROUTER_ID_1),
					Error::<Runtime>::ProofNotExpectedFromFirstRouter
				);
			});
		}
	}

	mod increment_proof_count {
		use super::*;

		#[test]
		fn success_same_session() {
			new_test_ext().execute_with(|| {
				let session_id = 1;
				let mut inbound_entry = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id,
					current_count: 1,
				});

				assert_ok!(inbound_entry.increment_proof_count(session_id));
				assert_eq!(
					inbound_entry,
					InboundEntry::<Runtime>::Proof(ProofEntry {
						session_id,
						current_count: 2,
					})
				);
			});
		}

		#[test]
		fn success_new_session() {
			new_test_ext().execute_with(|| {
				let session_id = 1;
				let mut inbound_entry = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id,
					current_count: 1,
				});

				let new_session_id = session_id + 1;

				assert_ok!(inbound_entry.increment_proof_count(new_session_id));
				assert_eq!(
					inbound_entry,
					InboundEntry::<Runtime>::Proof(ProofEntry {
						session_id: new_session_id,
						current_count: 1,
					})
				);
			});
		}

		#[test]
		fn expected_message_proof_type() {
			new_test_ext().execute_with(|| {
				let mut inbound_entry = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id: 1,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: Message::Simple,
					expected_proof_count: 1,
				});

				assert_noop!(
					inbound_entry.increment_proof_count(1),
					Error::<Runtime>::ExpectedMessageProofType
				);
			});
		}
	}

	mod pre_dispatch_update {
		use super::*;

		#[test]
		fn message_success_same_session() {
			new_test_ext().execute_with(|| {
				let mut inbound_entry_1 = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id: 1,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: Message::Simple,
					expected_proof_count: 2,
				});

				let inbound_entry_2 = inbound_entry_1.clone();

				assert_ok!(inbound_entry_1.pre_dispatch_update(inbound_entry_2));
				assert_eq!(
					inbound_entry_1,
					InboundEntry::<Runtime>::Message(MessageEntry {
						session_id: 1,
						domain_address: TEST_DOMAIN_ADDRESS,
						message: Message::Simple,
						expected_proof_count: 4,
					})
				)
			});
		}

		#[test]
		fn message_success_session_change() {
			new_test_ext().execute_with(|| {
				let mut inbound_entry_1 = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id: 1,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: Message::Simple,
					expected_proof_count: 2,
				});

				let inbound_entry_2 = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id: 2,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: Message::Simple,
					expected_proof_count: 5,
				});

				assert_ok!(inbound_entry_1.pre_dispatch_update(inbound_entry_2.clone()));
				assert_eq!(inbound_entry_1, inbound_entry_2)
			});
		}

		#[test]
		fn proof_success_same_session() {
			new_test_ext().execute_with(|| {
				let mut inbound_entry_1 = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id: 1,
					current_count: 1,
				});

				let inbound_entry_2 = inbound_entry_1.clone();

				assert_ok!(inbound_entry_1.pre_dispatch_update(inbound_entry_2));
				assert_eq!(
					inbound_entry_1,
					InboundEntry::<Runtime>::Proof(ProofEntry {
						session_id: 1,
						current_count: 2,
					})
				)
			});
		}

		#[test]
		fn proof_success_session_change() {
			new_test_ext().execute_with(|| {
				let mut inbound_entry_1 = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id: 1,
					current_count: 1,
				});

				let inbound_entry_2 = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id: 2,
					current_count: 3,
				});

				assert_ok!(inbound_entry_1.pre_dispatch_update(inbound_entry_2.clone()));
				assert_eq!(inbound_entry_1, inbound_entry_2)
			});
		}

		#[test]
		fn mismatch_errors() {
			new_test_ext().execute_with(|| {
				let mut inbound_entry_1 = InboundEntry::<Runtime>::Message(MessageEntry {
					session_id: 1,
					domain_address: TEST_DOMAIN_ADDRESS,
					message: Message::Simple,
					expected_proof_count: 2,
				});

				let mut inbound_entry_2 = InboundEntry::<Runtime>::Proof(ProofEntry {
					session_id: 1,
					current_count: 1,
				});

				assert_noop!(
					inbound_entry_1.pre_dispatch_update(inbound_entry_2.clone()),
					Error::<Runtime>::ExpectedMessageType
				);

				assert_noop!(
					inbound_entry_2.pre_dispatch_update(inbound_entry_1),
					Error::<Runtime>::ExpectedMessageProofType
				);
			});
		}
	}
}
