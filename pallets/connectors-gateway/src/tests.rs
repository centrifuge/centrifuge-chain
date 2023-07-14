use cfg_mocks::*;
use cfg_traits::connectors::{Codec, OutboundQueue};
use cfg_types::domain_address::*;
use frame_support::{assert_noop, assert_ok};
use sp_core::{crypto::AccountId32, ByteArray, H160};
use sp_runtime::DispatchError::BadOrigin;

use super::{
	mock::{RuntimeEvent as MockEvent, *},
	origin::*,
	pallet::*,
};

mod utils {
	use super::*;

	pub fn get_random_test_account_id() -> AccountId32 {
		rand::random::<[u8; 32]>().into()
	}

	pub fn event_exists<E: Into<MockEvent>>(e: E) {
		let actual: Vec<MockEvent> = frame_system::Pallet::<Runtime>::events()
			.iter()
			.map(|e| e.event.clone())
			.collect();

		let e: MockEvent = e.into();
		let mut exists = false;
		for evt in actual {
			if evt == e {
				exists = true;
				break;
			}
		}
		assert!(exists);
	}
}

use utils::*;

mod set_domain_router {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = DomainRouterMock::new();

			assert_ok!(ConnectorsGateway::set_domain_router(
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
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = DomainRouterMock::new();

			assert_noop!(
				ConnectorsGateway::set_domain_router(
					RuntimeOrigin::signed(get_random_test_account_id()),
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
			let router = DomainRouterMock::new();

			assert_noop!(
				ConnectorsGateway::set_domain_router(RuntimeOrigin::root(), domain.clone(), router),
				Error::<Runtime>::DomainNotSupported
			);

			let storage_entry = DomainRouters::<Runtime>::get(domain);
			assert!(storage_entry.is_none());
		});
	}
}

mod add_connector {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(ConnectorsGateway::add_connector(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(ConnectorsAllowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			event_exists(Event::<Runtime>::ConnectorAdded {
				connector: domain_address,
			});
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_noop!(
				ConnectorsGateway::add_connector(
					RuntimeOrigin::signed(get_random_test_account_id()),
					domain_address.clone(),
				),
				BadOrigin
			);

			assert!(!ConnectorsAllowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));
		});
	}

	#[test]
	fn unsupported_domain() {
		new_test_ext().execute_with(|| {
			let domain_address = DomainAddress::Centrifuge(get_random_test_account_id().into());

			assert_noop!(
				ConnectorsGateway::add_connector(RuntimeOrigin::root(), domain_address.clone()),
				Error::<Runtime>::DomainNotSupported
			);

			assert!(!ConnectorsAllowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));
		});
	}

	#[test]
	fn connector_already_added() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(ConnectorsGateway::add_connector(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(ConnectorsAllowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			assert_noop!(
				ConnectorsGateway::add_connector(RuntimeOrigin::root(), domain_address,),
				Error::<Runtime>::ConnectorAlreadyAdded
			);
		});
	}
}

mod remove_connector {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(ConnectorsGateway::add_connector(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(ConnectorsAllowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			event_exists(Event::<Runtime>::ConnectorAdded {
				connector: domain_address.clone(),
			});

			assert_ok!(ConnectorsGateway::remove_connector(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(!ConnectorsAllowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			event_exists(Event::<Runtime>::ConnectorAdded {
				connector: domain_address.clone(),
			});
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_noop!(
				ConnectorsGateway::remove_connector(
					RuntimeOrigin::signed(get_random_test_account_id()),
					domain_address.clone(),
				),
				BadOrigin
			);
		});
	}

	#[test]
	fn connector_not_found() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_noop!(
				ConnectorsGateway::remove_connector(RuntimeOrigin::root(), domain_address.clone(),),
				Error::<Runtime>::ConnectorNotFound,
			);
		});
	}
}

mod process_msg {
	use sp_core::bounded::BoundedVec;

	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(ConnectorsGateway::add_connector(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(ConnectorsAllowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			event_exists(Event::<Runtime>::ConnectorAdded {
				connector: domain_address.clone(),
			});

			let expected_msg = MessageMock::First;
			let encoded_msg = expected_msg.serialize();

			let expected_domain_address = domain_address.clone();

			MockConnectors::mock_submit(move |domain, message| {
				assert_eq!(domain, expected_domain_address);
				assert_eq!(message, expected_msg);
				Ok(())
			});

			assert_ok!(ConnectorsGateway::process_msg(
				GatewayOrigin::Local(domain_address).into(),
				BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
			));
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let encoded_msg = MessageMock::First.serialize();

			assert_noop!(
				ConnectorsGateway::process_msg(
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
			let domain_address = DomainAddress::Centrifuge(get_random_test_account_id().into());
			let encoded_msg = MessageMock::First.serialize();

			assert_noop!(
				ConnectorsGateway::process_msg(
					GatewayOrigin::Local(domain_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				Error::<Runtime>::InvalidMessageOrigin,
			);
		});
	}

	#[test]
	fn unknown_connector() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());
			let encoded_msg = MessageMock::First.serialize();

			assert_noop!(
				ConnectorsGateway::process_msg(
					GatewayOrigin::Local(domain_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				Error::<Runtime>::UnknownConnector,
			);
		});
	}

	#[test]
	fn message_decode() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(ConnectorsGateway::add_connector(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(ConnectorsAllowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			event_exists(Event::<Runtime>::ConnectorAdded {
				connector: domain_address.clone(),
			});

			let encoded_msg: Vec<u8> = vec![11];

			assert_noop!(
				ConnectorsGateway::process_msg(
					GatewayOrigin::Local(domain_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				Error::<Runtime>::MessageDecode,
			);
		});
	}

	#[test]
	fn connectors_error() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_random_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(ConnectorsGateway::add_connector(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			assert!(ConnectorsAllowlist::<Runtime>::contains_key(
				domain_address.domain(),
				domain_address.clone()
			));

			event_exists(Event::<Runtime>::ConnectorAdded {
				connector: domain_address.clone(),
			});

			let expected_msg = MessageMock::First;
			let encoded_msg = expected_msg.serialize();

			let expected_domain_address = domain_address.clone();

			let err = sp_runtime::DispatchError::from("connectors error");

			MockConnectors::mock_submit(move |domain, message| {
				assert_eq!(domain, expected_domain_address);
				assert_eq!(message, expected_msg);
				Err(err)
			});

			assert_noop!(
				ConnectorsGateway::process_msg(
					GatewayOrigin::Local(domain_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				err,
			);
		});
	}
}

mod outbound_queue_impl {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = DomainRouterMock::new();

			assert_ok!(ConnectorsGateway::set_domain_router(
				RuntimeOrigin::root(),
				domain.clone(),
				router.clone(),
			));

			let storage_entry = DomainRouters::<Runtime>::get(domain.clone());
			assert_eq!(storage_entry.unwrap(), router);

			event_exists(Event::<Runtime>::DomainRouterSet {
				domain: domain.clone(),
				router,
			});

			let sender = get_random_test_account_id();
			let msg = MessageMock::First;

			assert_ok!(ConnectorsGateway::submit(sender, domain, msg));
		});
	}

	#[test]
	fn local_domain() {
		new_test_ext().execute_with(|| {
			let domain = Domain::Centrifuge;
			let sender = get_random_test_account_id();
			let msg = MessageMock::First;

			assert_noop!(
				ConnectorsGateway::submit(sender, domain, msg),
				Error::<Runtime>::DomainNotSupported
			);
		});
	}

	#[test]
	fn router_not_found() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let sender = get_random_test_account_id();
			let msg = MessageMock::First;

			assert_noop!(
				ConnectorsGateway::submit(sender, domain, msg),
				Error::<Runtime>::RouterNotFound
			);
		});
	}
}
