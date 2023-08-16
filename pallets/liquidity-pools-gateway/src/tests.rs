use cfg_mocks::*;
use cfg_traits::liquidity_pools::{Codec, OutboundQueue};
use cfg_types::domain_address::*;
use frame_support::{assert_noop, assert_ok};
use sp_core::{crypto::AccountId32, ByteArray, H160};
use sp_runtime::{DispatchError, DispatchError::BadOrigin};

use super::{
	mock::{RuntimeEvent as MockEvent, *},
	origin::*,
	pallet::*,
};

mod utils {
	use super::*;

	pub fn get_test_account_id() -> AccountId32 {
		[0u8; 32].into()
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
				LiquidityPoolsGateway::add_instance(RuntimeOrigin::root(), domain_address,),
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

mod process_msg {
	use sp_core::bounded::BoundedVec;

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

			let expected_msg = MessageMock::First;
			let encoded_msg = expected_msg.serialize();

			let expected_domain_address = domain_address.clone();

			MockLiquidityPools::mock_submit(move |domain, message| {
				assert_eq!(domain, expected_domain_address);
				assert_eq!(message, expected_msg);
				Ok(())
			});

			assert_ok!(LiquidityPoolsGateway::process_msg(
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
			let encoded_msg = MessageMock::First.serialize();

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
					GatewayOrigin::Local(domain_address).into(),
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
			let encoded_msg = MessageMock::First.serialize();

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
					GatewayOrigin::Local(domain_address).into(),
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
					GatewayOrigin::Local(domain_address).into(),
					BoundedVec::<u8, MaxIncomingMessageSize>::try_from(encoded_msg).unwrap()
				),
				Error::<Runtime>::MessageDecodingFailed,
			);
		});
	}

	#[test]
	fn liquidity_pools_error() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let expected_msg = MessageMock::First;
			let encoded_msg = expected_msg.serialize();

			let expected_domain_address = domain_address.clone();

			let err = sp_runtime::DispatchError::from("liquidity_pools error");

			MockLiquidityPools::mock_submit(move |domain, message| {
				assert_eq!(domain, expected_domain_address);
				assert_eq!(message, expected_msg);
				Err(err)
			});

			assert_noop!(
				LiquidityPoolsGateway::process_msg(
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
			let router = RouterMock::<Runtime>::default();
			router.mock_init(move || Ok(()));

			assert_ok!(LiquidityPoolsGateway::set_domain_router(
				RuntimeOrigin::root(),
				domain.clone(),
				router.clone(),
			));

			let sender = get_test_account_id();
			let msg = MessageMock::First;

			router.mock_send({
				let sender = sender.clone();
				let msg = msg.clone();

				move |mock_sender, mock_msg| {
					assert_eq!(sender, mock_sender);
					assert_eq!(msg, mock_msg);

					Ok(())
				}
			});

			assert_ok!(LiquidityPoolsGateway::submit(sender, domain, msg));
		});
	}

	#[test]
	fn router_error() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = RouterMock::<Runtime>::default();
			router.mock_init(move || Ok(()));

			assert_ok!(LiquidityPoolsGateway::set_domain_router(
				RuntimeOrigin::root(),
				domain.clone(),
				router.clone(),
			));

			let sender = get_test_account_id();
			let msg = MessageMock::First;
			let expected_error = DispatchError::Other("router error");

			router.mock_send({
				let sender = sender.clone();
				let msg = msg.clone();

				move |mock_sender, mock_msg| {
					assert_eq!(sender, mock_sender);
					assert_eq!(msg, mock_msg);

					Err(expected_error)
				}
			});

			assert_noop!(
				LiquidityPoolsGateway::submit(sender, domain, msg),
				expected_error,
			);
		});
	}

	#[test]
	fn local_domain() {
		new_test_ext().execute_with(|| {
			let domain = Domain::Centrifuge;
			let sender = get_test_account_id();
			let msg = MessageMock::First;

			assert_noop!(
				LiquidityPoolsGateway::submit(sender, domain, msg),
				Error::<Runtime>::DomainNotSupported
			);
		});
	}

	#[test]
	fn router_not_found() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let sender = get_test_account_id();
			let msg = MessageMock::First;

			assert_noop!(
				LiquidityPoolsGateway::submit(sender, domain, msg),
				Error::<Runtime>::RouterNotFound
			);
		});
	}
}
