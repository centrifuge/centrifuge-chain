/*
use cfg_mocks::*;
use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{LPEncoding, MessageProcessor, OutboundMessageHandler};
use cfg_types::domain_address::*;
use frame_support::{
	assert_err, assert_noop, assert_ok, dispatch::PostDispatchInfo, pallet_prelude::Pays,
	weights::Weight,
};
use sp_core::{bounded::BoundedVec, crypto::AccountId32, ByteArray, H160};
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
use crate::GatewayMessage;

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

			let gateway_message = GatewayMessage::<AccountId32, Message>::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
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

			let gateway_message = GatewayMessage::<AccountId32, Message>::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
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

			let router = RouterMock::<Runtime>::default();
			router.mock_init(move || Ok(()));

			assert_ok!(LiquidityPoolsGateway::set_domain_router(
				RuntimeOrigin::root(),
				domain.clone(),
				router.clone(),
			));

			let gateway_message = GatewayMessage::<AccountId32, Message>::Outbound {
				sender: <Runtime as Config>::Sender::get(),
				destination: domain.clone(),
				message: msg.clone(),
			};

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_msg| {
				assert_eq!(mock_msg, gateway_message);

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

			let router = RouterMock::<Runtime>::default();
			router.mock_init(move || Ok(()));

			assert_ok!(LiquidityPoolsGateway::set_domain_router(
				RuntimeOrigin::root(),
				domain.clone(),
				router.clone(),
			));

			let gateway_message = GatewayMessage::<AccountId32, Message>::Outbound {
				sender: <Runtime as Config>::Sender::get(),
				destination: domain.clone(),
				message: msg.clone(),
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

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let domain_address = DomainAddress::EVM(1, [1; 20]);
				let message = Message::Simple;
				let gateway_message = GatewayMessage::<AccountId32, Message>::Inbound {
					domain_address: domain_address.clone(),
					message: message.clone(),
				};

				MockLiquidityPools::mock_handle(move |mock_domain_address, mock_mesage| {
					assert_eq!(mock_domain_address, domain_address);
					assert_eq!(mock_mesage, message);

					Ok(())
				});

				let (res, _) = LiquidityPoolsGateway::process(gateway_message);
				assert_ok!(res);
			});
		}

		#[test]
		fn inbound_message_handler_error() {
			new_test_ext().execute_with(|| {
				let domain_address = DomainAddress::EVM(1, [1; 20]);
				let message = Message::Simple;
				let gateway_message = GatewayMessage::<AccountId32, Message>::Inbound {
					domain_address: domain_address.clone(),
					message: message.clone(),
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

				let router_mock = RouterMock::<Runtime>::default();
				router_mock.mock_send(move |mock_sender, mock_message| {
					assert_eq!(mock_sender, expected_sender);
					assert_eq!(mock_message, expected_message.serialize());

					Ok(router_post_info)
				});

				DomainRouters::<Runtime>::insert(domain.clone(), router_mock);

				let min_expected_weight = <Runtime as frame_system::Config>::DbWeight::get()
					.reads(1) + router_post_info.actual_weight.unwrap()
					+ Weight::from_parts(0, message.serialize().len() as u64);

				let gateway_message = GatewayMessage::<AccountId32, Message>::Outbound {
					sender,
					destination: domain,
					message: message.clone(),
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
				let domain = Domain::EVM(1);
				let message = Message::Simple;

				let expected_weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				let gateway_message = GatewayMessage::<AccountId32, Message>::Outbound {
					sender,
					destination: domain,
					message,
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

				let gateway_message = GatewayMessage::<AccountId32, Message>::Outbound {
					sender,
					destination: domain,
					message: message.clone(),
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
			});

			// 2 correct messages and 1 failed message processed.
			assert_eq!(weight, LP_DEFENSIVE_WEIGHT * 3);
			assert_err!(result, DispatchError::Unavailable);
		});
	}
}
*/
