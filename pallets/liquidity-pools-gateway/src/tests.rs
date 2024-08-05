use cfg_mocks::*;
use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use cfg_traits::liquidity_pools::{
	test_util::Message, LPEncoding, MessageProcessor, OutboundMessageHandler,
};
use cfg_types::domain_address::*;
use frame_support::{
	assert_noop, assert_ok, dispatch::PostDispatchInfo, pallet_prelude::Pays, weights::Weight,
};
use parity_scale_codec::MaxEncodedLen;
use sp_core::{bounded::BoundedVec, crypto::AccountId32, ByteArray, H160};
use sp_runtime::{DispatchError, DispatchError::BadOrigin, DispatchErrorWithPostInfo};

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

mod pallet_internals {

	use super::*;

	#[test]
	fn try_range_fails_if_slice_to_short() {
		new_test_ext().execute_with(|| {
			let three_bytes = [0u8; 3];
			let steps = 4usize;

			assert_noop!(
				Pallet::<Runtime>::try_range(
					&mut three_bytes.as_slice(),
					steps,
					Error::<Runtime>::MessageDecodingFailed,
					|_| Ok(())
				),
				Error::<Runtime>::MessageDecodingFailed
			);
		})
	}

	#[test]
	fn try_range_updates_slice_ref_correctly() {
		new_test_ext().execute_with(|| {
			let bytes = [1, 2, 3, 4, 5, 6, 7u8];
			let slice = &mut bytes.as_slice();
			let steps = 4;
			let first_section = Pallet::<Runtime>::try_range(
				slice,
				steps,
				Error::<Runtime>::MessageDecodingFailed,
				|first_section| Ok(first_section),
			)
			.expect("Slice is long enough");

			assert_eq!(first_section, &[1, 2, 3, 4]);

			let steps = 2;
			let second_section = Pallet::<Runtime>::try_range(
				slice,
				steps,
				Error::<Runtime>::MessageDecodingFailed,
				|second_section| Ok(second_section),
			)
			.expect("Slice is long enough");

			assert_eq!(&second_section, &[5, 6]);

			let steps = 1;
			let third_section = Pallet::<Runtime>::try_range(
				slice,
				steps,
				Error::<Runtime>::MessageDecodingFailed,
				|third_section| Ok(third_section),
			)
			.expect("Slice is long enough");

			assert_eq!(&third_section, &[7]);
		})
	}

	#[test]
	fn try_range_does_not_update_slice_if_transformer_errors() {
		new_test_ext().execute_with(|| {
			let bytes = [1, 2, 3, 4, 5, 6, 7u8];
			let slice = &mut bytes.as_slice();
			let steps = 4;
			let first_section = Pallet::<Runtime>::try_range(
				slice,
				steps,
				Error::<Runtime>::MessageDecodingFailed,
				|first_section| Ok(first_section),
			)
			.expect("Slice is long enough");

			assert_eq!(first_section, &[1, 2, 3, 4]);

			let steps = 1;
			assert!(Pallet::<Runtime>::try_range(
				slice,
				steps,
				Error::<Runtime>::MessageDecodingFailed,
				|_| Err::<(), _>(DispatchError::Corruption)
			)
			.is_err());
			assert_eq!(slice, &[5, 6, 7]);
		})
	}
}

mod set_domain_router {
	use super::*;

	#[test]
	fn success_with_root() {
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
	fn success_with_lp_admin_account() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = RouterMock::<Runtime>::default();
			router.mock_init(move || Ok(()));

			assert_ok!(LiquidityPoolsGateway::set_domain_router(
				RuntimeOrigin::signed(LP_ADMIN_ACCOUNT),
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
	fn success_with_root() {
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
	fn success_with_lp_admin_account() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::signed(LP_ADMIN_ACCOUNT),
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
				LiquidityPoolsGateway::add_instance(RuntimeOrigin::root(), domain_address,),
				Error::<Runtime>::InstanceAlreadyAdded
			);
		});
	}
}

mod remove_instance {
	use super::*;

	#[test]
	fn success_with_root() {
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
	fn success_with_lp_admin_account() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::signed(LP_ADMIN_ACCOUNT),
				domain_address.clone(),
			));

			assert_ok!(LiquidityPoolsGateway::remove_instance(
				RuntimeOrigin::signed(LP_ADMIN_ACCOUNT),
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

mod process_msg_axelar_relay {
	use sp_core::bounded::BoundedVec;

	use super::*;
	use crate::RelayerMessageDecodingError;

	#[test]
	fn success_from_solidity_payload() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let source_address = hex_literal::hex!["8503b4452Bf6238cC76CdbEE223b46d7196b1c93"];
			let domain_address = DomainAddress::EVM(SOURCE_CHAIN_EVM_ID, source_address);
			let relayer_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert_ok!(LiquidityPoolsGateway::add_relayer(
				RuntimeOrigin::root(),
				relayer_address.clone(),
			));


			let expected_msg = Message;
			let expected_domain_address = domain_address.clone();

			let inbound_message = GatewayMessage::<AccountId32, Message>::Inbound { domain_address: expected_domain_address, message: expected_msg };

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_message| {
				assert_eq!(mock_message, inbound_message);
				Ok(())
			});

			let expected_domain_address = domain_address.clone();

			MockOriginRecovery::mock_try_convert(move |origin| {
				let (source_chain, origin_source_address) = origin;

				assert_eq!(&source_chain, SOURCE_CHAIN.as_slice());
				assert_eq!(&origin_source_address, source_address.as_slice());

				Ok(expected_domain_address.clone())
			});

            let solidity_header = "0000000a657468657265756d2d320000002a307838353033623434353242663632333863433736436462454532323362343664373139366231633933";
			let payload = [hex::decode(solidity_header).unwrap(), Message.serialize()].concat();

			assert_ok!(LiquidityPoolsGateway::process_msg(
				GatewayOrigin::AxelarRelay(relayer_address).into(),
				BoundedVec::<u8, MaxIncomingMessageSize>::try_from(payload).unwrap()
			));
		})
	}

	#[test]
	fn success_with_root() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(SOURCE_CHAIN_EVM_ID, SOURCE_ADDRESS);
			let relayer_address = DomainAddress::EVM(0, address.into());
			let message = Message;
			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert_ok!(LiquidityPoolsGateway::add_relayer(
				RuntimeOrigin::root(),
				relayer_address.clone(),
			));

			let gateway_message = GatewayMessage::<AccountId32, Message>::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
			};

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_message| {
				assert_eq!(mock_message, gateway_message);
				Ok(())
			});

			let expected_domain_address = domain_address.clone();

			MockOriginRecovery::mock_try_convert(move |origin| {
				let (source_chain, source_address) = origin;

				assert_eq!(&source_chain, SOURCE_CHAIN.as_slice());
				assert_eq!(&source_address, SOURCE_ADDRESS.as_slice());

				Ok(expected_domain_address.clone())
			});

			let mut msg = Vec::new();
			msg.extend_from_slice(&(LENGTH_SOURCE_CHAIN as u32).to_be_bytes());
			msg.extend_from_slice(&SOURCE_CHAIN);
			msg.extend_from_slice(&(LENGTH_SOURCE_ADDRESS as u32).to_be_bytes());
			msg.extend_from_slice(&SOURCE_ADDRESS);
			msg.extend_from_slice(&message.serialize());

			assert_ok!(LiquidityPoolsGateway::process_msg(
				GatewayOrigin::AxelarRelay(relayer_address).into(),
				BoundedVec::<u8, MaxIncomingMessageSize>::try_from(msg).unwrap()
			));
		});
	}

	#[test]
	fn success_with_lp_admin_account() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(SOURCE_CHAIN_EVM_ID, SOURCE_ADDRESS);
			let relayer_address = DomainAddress::EVM(0, address.into());
			let message = Message;

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::signed(LP_ADMIN_ACCOUNT),
				domain_address.clone(),
			));

			assert_ok!(LiquidityPoolsGateway::add_relayer(
				RuntimeOrigin::signed(LP_ADMIN_ACCOUNT),
				relayer_address.clone(),
			));

			let gateway_message = GatewayMessage::<AccountId32, Message>::Inbound {
				domain_address: domain_address.clone(),
				message: message.clone(),
			};

			MockLiquidityPoolsGatewayQueue::mock_submit(move |mock_message| {
				assert_eq!(mock_message, gateway_message);
				Ok(())
			});

			let expected_domain_address = domain_address.clone();

			MockOriginRecovery::mock_try_convert(move |origin| {
				let (source_chain, source_address) = origin;

				assert_eq!(&source_chain, SOURCE_CHAIN.as_slice());
				assert_eq!(&source_address, SOURCE_ADDRESS.as_slice());

				Ok(expected_domain_address.clone())
			});

			let mut msg = Vec::new();
			msg.extend_from_slice(&(LENGTH_SOURCE_CHAIN as u32).to_be_bytes());
			msg.extend_from_slice(&SOURCE_CHAIN);
			msg.extend_from_slice(&(LENGTH_SOURCE_ADDRESS as u32).to_be_bytes());
			msg.extend_from_slice(&SOURCE_ADDRESS);
			msg.extend_from_slice(&message.serialize());

			assert_ok!(LiquidityPoolsGateway::process_msg(
				GatewayOrigin::AxelarRelay(relayer_address).into(),
				BoundedVec::<u8, MaxIncomingMessageSize>::try_from(msg).unwrap()
			));
		});
	}

	#[test]
	fn invalid_message_origin() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::Centrifuge(get_test_account_id().into());
			let relayer_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_relayer(
				RuntimeOrigin::root(),
				relayer_address.clone(),
			));

			let expected_msg = Message;

			let mut msg = Vec::new();

			// Need to prepend length signaler
			msg.extend_from_slice(&(0 as u32).to_be_bytes());
			msg.extend_from_slice(&(0 as u32).to_be_bytes());
			msg.extend_from_slice(&expected_msg.serialize());

			MockOriginRecovery::mock_try_convert(move |origin| {
				let (source_chain, source_address) = origin;

				assert!(source_chain.is_empty());
				assert!(source_address.is_empty());

				Ok(domain_address.clone())
			});

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
					GatewayOrigin::AxelarRelay(relayer_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(msg).unwrap()
				),
				Error::<Runtime>::RelayerMessageDecodingFailed {
					reason: RelayerMessageDecodingError::MalformedSourceAddress
				},
			);
		});
	}

	#[test]
	fn unknown_instance() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(SOURCE_CHAIN_EVM_ID, SOURCE_ADDRESS);
			let relayer_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_relayer(
				RuntimeOrigin::root(),
				relayer_address.clone(),
			));

			let expected_msg = Message;

			let mut msg = Vec::new();
			msg.extend_from_slice(&(LENGTH_SOURCE_CHAIN as u32).to_be_bytes());
			msg.extend_from_slice(&SOURCE_CHAIN);
			msg.extend_from_slice(&(LENGTH_SOURCE_ADDRESS as u32).to_be_bytes());
			msg.extend_from_slice(&SOURCE_ADDRESS);
			msg.extend_from_slice(&expected_msg.serialize());

			let expected_domain_address = domain_address.clone();

			MockOriginRecovery::mock_try_convert(move |origin| {
				let (source_chain, source_address) = origin;

				assert_eq!(&source_chain, SOURCE_CHAIN.as_slice());
				assert_eq!(&source_address, SOURCE_ADDRESS.as_slice());

				Ok(expected_domain_address.clone())
			});

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
					GatewayOrigin::AxelarRelay(relayer_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(msg).unwrap()
				),
				Error::<Runtime>::UnknownInstance
			);
		});
	}

	#[test]
	fn message_decode() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(SOURCE_CHAIN_EVM_ID, SOURCE_ADDRESS);
			let relayer_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert_ok!(LiquidityPoolsGateway::add_relayer(
				RuntimeOrigin::root(),
				relayer_address.clone(),
			));

			let encoded_msg: Vec<u8> = vec![11];
			let expected_domain_address = domain_address.clone();

			MockOriginRecovery::mock_try_convert(move |origin| {
				let (source_chain, source_address) = origin;

				assert_eq!(&source_chain, SOURCE_CHAIN.as_slice());
				assert_eq!(&source_address, SOURCE_ADDRESS.as_slice());

				Ok(expected_domain_address.clone())
			});

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
					GatewayOrigin::Domain(domain_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				Error::<Runtime>::MessageDecodingFailed,
			);
		});
	}

	#[test]
	fn message_queue_error() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(SOURCE_CHAIN_EVM_ID, SOURCE_ADDRESS);
			let relayer_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert_ok!(LiquidityPoolsGateway::add_relayer(
				RuntimeOrigin::root(),
				relayer_address.clone(),
			));

			let expected_msg = Message;
			let encoded_msg = expected_msg.serialize();

			let expected_domain_address = domain_address.clone();

			MockOriginRecovery::mock_try_convert(move |_| Ok(expected_domain_address.clone()));

			let err = sp_runtime::DispatchError::from("message queue error");

			MockLiquidityPoolsGatewayQueue::mock_submit(move |_| Err(err));

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
					GatewayOrigin::Domain(domain_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				err,
			);
		});
	}
}

mod process_msg_domain {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());
			let message = Message;

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

			assert_ok!(LiquidityPoolsGateway::process_msg(
				GatewayOrigin::Domain(domain_address).into(),
				BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
			));
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let encoded_msg = Message.serialize();

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
					RuntimeOrigin::root(),
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
			let encoded_msg = Message.serialize();

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
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
			let encoded_msg = Message.serialize();

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
					GatewayOrigin::Domain(domain_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				Error::<Runtime>::UnknownInstance,
			);
		});
	}

	#[test]
	fn message_decode() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let encoded_msg: Vec<u8> = vec![11];

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
					GatewayOrigin::Domain(domain_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				Error::<Runtime>::MessageDecodingFailed,
			);
		});
	}

	#[test]
	fn message_queue_error() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());
			let message = Message;

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
				LiquidityPoolsGateway::process_msg(
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
			let msg = Message;

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
			let msg = Message;

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
			let msg = Message;

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
	fn success_with_root() {
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
	fn success_with_lp_admin_account() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);

			assert_ok!(LiquidityPoolsGateway::set_domain_hook_address(
				RuntimeOrigin::signed(LP_ADMIN_ACCOUNT),
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
				let message = Message;
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
				let message = Message;
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

				let expected_weight = Weight::from_parts(0, Message::max_encoded_len() as u64)
					.saturating_add(LP_DEFENSIVE_WEIGHT);

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_noop!(res, err);
				assert_eq!(weight, expected_weight);
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
				let message = Message;

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

				let mut expected_weight =
					<Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Pallet::<Runtime>::update_total_post_dispatch_info_weight(
					&mut expected_weight,
					router_post_info.actual_weight,
				);

				let gateway_message = GatewayMessage::<AccountId32, Message>::Outbound {
					sender,
					destination: domain,
					message,
				};

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_ok!(res);
				assert_eq!(weight, expected_weight);
			});
		}

		#[test]
		fn router_not_found() {
			new_test_ext().execute_with(|| {
				let sender = get_test_account_id();
				let domain = Domain::EVM(1);
				let message = Message;

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
				let message = Message;

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

				let mut expected_weight =
					<Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Pallet::<Runtime>::update_total_post_dispatch_info_weight(
					&mut expected_weight,
					router_post_info.actual_weight,
				);

				let gateway_message = GatewayMessage::<AccountId32, Message>::Outbound {
					sender,
					destination: domain,
					message,
				};

				let (res, weight) = LiquidityPoolsGateway::process(gateway_message);
				assert_noop!(res, router_err);
				assert_eq!(weight, expected_weight);
			});
		}
	}
}
