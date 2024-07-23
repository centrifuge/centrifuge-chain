use cfg_mocks::*;
use cfg_primitives::OutboundMessageNonce;
use cfg_traits::liquidity_pools::{test_util::Message, LPEncoding};
use cfg_types::domain_address::*;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::{Pays, PostDispatchInfo},
	weights::Weight,
};
use sp_core::{crypto::AccountId32, ByteArray, H160};
use sp_runtime::{
	traits::{One, Zero},
	BoundedVec, DispatchError,
	DispatchError::BadOrigin,
	DispatchErrorWithPostInfo,
};

use super::{
	mock::{RuntimeEvent as MockEvent, *},
	origin::*,
	pallet::*,
};

mod utils {
	use super::*;

	pub fn get_mock_routers(
	) -> BoundedVec<<Runtime as Config>::Router, <Runtime as Config>::MaxRouterCount> {
		let mut routers = Vec::new();

		for _ in 0..<Runtime as Config>::MaxRouterCount::get() {
			let router = RouterMock::<Runtime>::default();
			router.mock_init(move || Ok(()));

			routers.push(router);
		}

		BoundedVec::<_, _>::try_from(routers).expect("can create multi-router vec")
	}

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

			assert_eq!(
				DomainRouters::<Runtime>::get(domain.clone()).unwrap(),
				router
			);

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

			assert!(DomainRouters::<Runtime>::get(domain).is_none());
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

			assert!(DomainRouters::<Runtime>::get(domain).is_none());
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

			assert!(DomainRouters::<Runtime>::get(domain).is_none());
		});
	}
}

mod set_domain_multi_router {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let routers = get_mock_routers();

			assert_ok!(LiquidityPoolsGateway::set_domain_multi_router(
				RuntimeOrigin::root(),
				domain.clone(),
				routers.clone(),
			));

			assert_eq!(
				DomainMultiRouters::<Runtime>::get(domain.clone()).unwrap(),
				routers
			);

			event_exists(Event::<Runtime>::DomainMultiRouterSet { domain, routers });
		});
	}
	#[test]
	fn router_init_error() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = RouterMock::<Runtime>::default();
			router.mock_init(move || Err(DispatchError::Other("error")));

			let routers = BoundedVec::try_from(vec![router]).unwrap();

			assert_noop!(
				LiquidityPoolsGateway::set_domain_multi_router(
					RuntimeOrigin::root(),
					domain.clone(),
					routers,
				),
				Error::<Runtime>::RouterInitFailed,
			);

			assert!(DomainMultiRouters::<Runtime>::get(domain).is_none());
		});
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let domain = Domain::EVM(0);
			let router = RouterMock::<Runtime>::default();

			let routers = BoundedVec::try_from(vec![router]).unwrap();

			assert_noop!(
				LiquidityPoolsGateway::set_domain_multi_router(
					RuntimeOrigin::signed(get_test_account_id()),
					domain.clone(),
					routers,
				),
				BadOrigin
			);

			assert!(DomainMultiRouters::<Runtime>::get(domain).is_none());
		});
	}

	#[test]
	fn unsupported_domain() {
		new_test_ext().execute_with(|| {
			let domain = Domain::Centrifuge;
			let router = RouterMock::<Runtime>::default();

			let routers = BoundedVec::try_from(vec![router]).unwrap();

			assert_noop!(
				LiquidityPoolsGateway::set_domain_multi_router(
					RuntimeOrigin::root(),
					domain.clone(),
					routers,
				),
				Error::<Runtime>::DomainNotSupported
			);

			assert!(DomainMultiRouters::<Runtime>::get(domain).is_none());
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

mod process_msg_axelar_relay {
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

			MockLiquidityPools::mock_handle(move |domain, message| {
				assert_eq!(domain, expected_domain_address);
				assert_eq!(message, expected_msg);
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
	fn success() {
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
			let expected_domain_address = domain_address.clone();

			let mut msg = Vec::new();
			msg.extend_from_slice(&(LENGTH_SOURCE_CHAIN as u32).to_be_bytes());
			msg.extend_from_slice(&SOURCE_CHAIN);
			msg.extend_from_slice(&(LENGTH_SOURCE_ADDRESS as u32).to_be_bytes());
			msg.extend_from_slice(&SOURCE_ADDRESS);
			msg.extend_from_slice(&expected_msg.serialize());

			MockLiquidityPools::mock_handle(move |domain, message| {
				assert_eq!(domain, expected_domain_address);
				assert_eq!(message, expected_msg);
				Ok(())
			});

			let expected_domain_address = domain_address.clone();

			MockOriginRecovery::mock_try_convert(move |origin| {
				let (source_chain, source_address) = origin;

				assert_eq!(&source_chain, SOURCE_CHAIN.as_slice());
				assert_eq!(&source_address, SOURCE_ADDRESS.as_slice());

				Ok(expected_domain_address.clone())
			});

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
	fn liquidity_pools_error() {
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

			let err = sp_runtime::DispatchError::from("liquidity_pools error");

			MockLiquidityPools::mock_handle(move |_, _| Err(err));

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

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let expected_msg = Message;
			let encoded_msg = expected_msg.serialize();

			let expected_domain_address = domain_address.clone();

			MockLiquidityPools::mock_handle(move |domain, message| {
				assert_eq!(domain, expected_domain_address);
				assert_eq!(message, expected_msg);
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
	fn liquidity_pools_error() {
		new_test_ext().execute_with(|| {
			let address = H160::from_slice(&get_test_account_id().as_slice()[..20]);
			let domain_address = DomainAddress::EVM(0, address.into());

			assert_ok!(LiquidityPoolsGateway::add_instance(
				RuntimeOrigin::root(),
				domain_address.clone(),
			));

			let expected_msg = Message;
			let encoded_msg = expected_msg.serialize();

			let expected_domain_address = domain_address.clone();

			let err = sp_runtime::DispatchError::from("liquidity_pools error");

			MockLiquidityPools::mock_handle(move |domain, message| {
				assert_eq!(domain, expected_domain_address);
				assert_eq!(message, expected_msg);
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

//TODO(cdamian): Move to gateway queue

// mod process_outbound_message {
// 	use super::*;
//
// 	#[test]
// 	fn success() {
// 		new_test_ext().execute_with(|| {
// 			let domain = Domain::EVM(0);
// 			let sender = get_test_account_id();
// 			let msg = Message;
//
// 			let routers = get_mock_routers();
//
// 			let mock_fn = {
// 				let sender = sender.clone();
// 				let msg = msg.clone();
//
// 				move |mock_sender, mock_msg| {
// 					assert_eq!(sender, mock_sender);
// 					assert_eq!(msg.serialize(), mock_msg);
//
// 					Ok(PostDispatchInfo {
// 						actual_weight: Some(Weight::from_parts(100, 100)),
// 						pays_fee: Pays::Yes,
// 					})
// 				}
// 			};
//
// 			for router in &routers {
// 				router.mock_send(mock_fn.clone());
// 			}
//
// 			assert_ok!(LiquidityPoolsGateway::set_domain_multi_router(
// 				RuntimeOrigin::root(),
// 				domain.clone(),
// 				routers,
// 			));
//
// 			let nonce = OutboundMessageNonce::one();
//
// 			OutboundMessageQueue::<Runtime>::insert(
// 				nonce,
// 				(domain.clone(), sender.clone(), msg.clone()),
// 			);
//
// 			assert_ok!(LiquidityPoolsGateway::process_outbound_message(
// 				RuntimeOrigin::signed(sender.clone()),
// 				nonce
// 			));
//
// 			assert!(!OutboundMessageQueue::<Runtime>::contains_key(nonce));
//
// 			event_exists(Event::<Runtime>::OutboundMessageExecutionSuccess {
// 				nonce,
// 				sender,
// 				domain,
// 				message: msg,
// 			});
// 		});
// 	}
//
// 	#[test]
// 	fn message_not_found() {
// 		new_test_ext().execute_with(|| {
// 			assert_noop!(
// 				LiquidityPoolsGateway::process_outbound_message(
// 					RuntimeOrigin::signed(get_test_account_id()),
// 					OutboundMessageNonce::zero(),
// 				),
// 				Error::<Runtime>::OutboundMessageNotFound,
// 			);
// 		});
// 	}
//
// 	#[test]
// 	fn multi_router_not_found() {
// 		new_test_ext().execute_with(|| {
// 			let domain = Domain::EVM(0);
// 			let sender = get_test_account_id();
// 			let msg = Message;
//
// 			let nonce = OutboundMessageNonce::one();
//
// 			OutboundMessageQueue::<Runtime>::insert(
// 				nonce,
// 				(domain.clone(), sender.clone(), msg.clone()),
// 			);
//
// 			assert_ok!(LiquidityPoolsGateway::process_outbound_message(
// 				RuntimeOrigin::signed(get_test_account_id()),
// 				nonce,
// 			));
//
// 			let (stored_domain, stored_sender, stored_message, stored_error): (
// 				_,
// 				AccountId32,
// 				Message,
// 				_,
// 			) = FailedOutboundMessages::<Runtime>::get(nonce).unwrap();
//
// 			assert_eq!(stored_domain, domain);
// 			assert_eq!(stored_sender, sender);
// 			assert_eq!(stored_message, msg);
// 			assert_eq!(stored_error, Error::<Runtime>::MultiRouterNotFound.into());
// 		});
// 	}
//
// 	#[test]
// 	fn failure() {
// 		new_test_ext().execute_with(|| {
// 			let domain = Domain::EVM(0);
//
// 			let routers = get_mock_routers();
//
// 			assert_ok!(LiquidityPoolsGateway::set_domain_multi_router(
// 				RuntimeOrigin::root(),
// 				domain.clone(),
// 				routers.clone(),
// 			));
//
// 			let sender = get_test_account_id();
// 			let msg = Message;
// 			let err = DispatchError::Unavailable;
//
// 			routers[0].mock_send(|_, _| {
// 				Ok(PostDispatchInfo {
// 					actual_weight: Some(Weight::from_parts(100, 100)),
// 					pays_fee: Pays::Yes,
// 				})
// 			});
//
// 			routers[1].mock_send({
// 				let sender = sender.clone();
// 				let msg = msg.clone();
// 				let err = err.clone();
//
// 				move |mock_sender, mock_msg| {
// 					assert_eq!(sender, mock_sender);
// 					assert_eq!(msg.serialize(), mock_msg);
//
// 					Err(DispatchErrorWithPostInfo {
// 						post_info: PostDispatchInfo {
// 							actual_weight: Some(Weight::from_parts(100, 100)),
// 							pays_fee: Pays::Yes,
// 						},
// 						error: err,
// 					})
// 				}
// 			});
//
// 			let nonce = OutboundMessageNonce::one();
//
// 			OutboundMessageQueue::<Runtime>::insert(
// 				nonce,
// 				(domain.clone(), sender.clone(), msg.clone()),
// 			);
//
// 			assert_ok!(LiquidityPoolsGateway::process_outbound_message(
// 				RuntimeOrigin::signed(sender.clone()),
// 				nonce
// 			));
//
// 			assert!(!OutboundMessageQueue::<Runtime>::contains_key(nonce));
//
// 			let failed_queue_entry = FailedOutboundMessages::<Runtime>::get(nonce)
// 				.expect("expected failed message queue entry");
//
// 			assert_eq!(
// 				failed_queue_entry,
// 				(domain.clone(), sender.clone(), msg.clone(), err.clone())
// 			);
//
// 			event_exists(Event::<Runtime>::OutboundMessageExecutionFailure {
// 				nonce,
// 				sender,
// 				domain,
// 				message: msg,
// 				error: err,
// 			});
// 		});
// 	}
// }

// mod process_failed_outbound_message {
// 	use super::*;
//
// 	#[test]
// 	fn success() {
// 		new_test_ext().execute_with(|| {
// 			let domain = Domain::EVM(0);
// 			let sender = get_test_account_id();
// 			let msg = Message;
// 			let err = DispatchError::Unavailable;
//
// 			let routers = get_mock_routers();
//
// 			let mock_fn = {
// 				let sender = sender.clone();
// 				let msg = msg.clone();
//
// 				move |mock_sender, mock_msg| {
// 					assert_eq!(sender, mock_sender);
// 					assert_eq!(msg.serialize(), mock_msg);
//
// 					Ok(PostDispatchInfo {
// 						actual_weight: Some(Weight::from_parts(100, 100)),
// 						pays_fee: Pays::Yes,
// 					})
// 				}
// 			};
//
// 			for router in &routers {
// 				router.mock_send(mock_fn.clone());
// 			}
//
// 			assert_ok!(LiquidityPoolsGateway::set_domain_multi_router(
// 				RuntimeOrigin::root(),
// 				domain.clone(),
// 				routers,
// 			));
//
// 			let nonce = OutboundMessageNonce::one();
//
// 			FailedOutboundMessages::<Runtime>::insert(
// 				nonce,
// 				(domain.clone(), sender.clone(), msg.clone(), err),
// 			);
//
// 			assert_ok!(LiquidityPoolsGateway::process_failed_outbound_message(
// 				RuntimeOrigin::signed(sender.clone()),
// 				nonce
// 			));
//
// 			assert!(FailedOutboundMessages::<Runtime>::get(nonce).is_none());
//
// 			event_exists(Event::<Runtime>::OutboundMessageExecutionSuccess {
// 				nonce,
// 				sender,
// 				domain,
// 				message: msg,
// 			});
// 		});
// 	}
//
// 	#[test]
// 	fn message_not_found() {
// 		new_test_ext().execute_with(|| {
// 			assert_noop!(
// 				LiquidityPoolsGateway::process_failed_outbound_message(
// 					RuntimeOrigin::signed(get_test_account_id()),
// 					OutboundMessageNonce::zero(),
// 				),
// 				Error::<Runtime>::OutboundMessageNotFound,
// 			);
// 		});
// 	}
//
// 	#[test]
// 	fn multi_router_not_found() {
// 		new_test_ext().execute_with(|| {
// 			let domain = Domain::EVM(0);
// 			let sender = get_test_account_id();
// 			let msg = Message;
// 			let err = DispatchError::Unavailable;
//
// 			let nonce = OutboundMessageNonce::one();
//
// 			FailedOutboundMessages::<Runtime>::insert(
// 				nonce,
// 				(domain.clone(), sender.clone(), msg.clone(), err),
// 			);
//
// 			assert_ok!(LiquidityPoolsGateway::process_failed_outbound_message(
// 				RuntimeOrigin::signed(get_test_account_id()),
// 				nonce,
// 			));
//
// 			assert!(FailedOutboundMessages::<Runtime>::get(nonce).is_some());
// 		});
// 	}
//
// 	#[test]
// 	fn failure() {
// 		new_test_ext().execute_with(|| {
// 			let domain = Domain::EVM(0);
//
// 			let routers = get_mock_routers();
//
// 			assert_ok!(LiquidityPoolsGateway::set_domain_multi_router(
// 				RuntimeOrigin::root(),
// 				domain.clone(),
// 				routers.clone(),
// 			));
//
// 			let sender = get_test_account_id();
// 			let msg = Message;
// 			let err = DispatchError::Unavailable;
//
// 			routers[0].mock_send(|_, _| {
// 				Ok(PostDispatchInfo {
// 					actual_weight: Some(Weight::from_parts(100, 100)),
// 					pays_fee: Pays::Yes,
// 				})
// 			});
// 			routers[1].mock_send({
// 				let sender = sender.clone();
// 				let msg = msg.clone();
// 				let err = err.clone();
//
// 				move |mock_sender, mock_msg| {
// 					assert_eq!(sender, mock_sender);
// 					assert_eq!(msg.serialize(), mock_msg);
//
// 					Err(DispatchErrorWithPostInfo {
// 						post_info: PostDispatchInfo {
// 							actual_weight: Some(Weight::from_parts(100, 100)),
// 							pays_fee: Pays::Yes,
// 						},
// 						error: err,
// 					})
// 				}
// 			});
//
// 			let nonce = OutboundMessageNonce::one();
//
// 			FailedOutboundMessages::<Runtime>::insert(
// 				nonce,
// 				(domain.clone(), sender.clone(), msg.clone(), err.clone()),
// 			);
//
// 			assert_ok!(LiquidityPoolsGateway::process_failed_outbound_message(
// 				RuntimeOrigin::signed(sender.clone()),
// 				nonce
// 			));
//
// 			assert!(FailedOutboundMessages::<Runtime>::contains_key(nonce));
//
// 			event_exists(Event::<Runtime>::OutboundMessageExecutionFailure {
// 				nonce,
// 				sender,
// 				domain,
// 				message: msg,
// 				error: err,
// 			});
// 		});
// 	}
// }

// mod outbound_queue_impl {
// 	use super::*;
//
// 	#[test]
// 	fn success() {
// 		new_test_ext().execute_with(|| {
// 			let domain = Domain::EVM(0);
// 			let sender = get_test_account_id();
// 			let msg = Message;
//
// 			let router = RouterMock::<Runtime>::default();
// 			router.mock_init(move || Ok(()));
//
// 			let routers = BoundedVec::<
// 				<Runtime as Config>::Router,
// 				<Runtime as Config>::MaxRouterCount,
// 			>::try_from(vec![router])
// 			.expect("can create multi-router vec");
//
// 			assert_ok!(LiquidityPoolsGateway::set_domain_multi_router(
// 				RuntimeOrigin::root(),
// 				domain.clone(),
// 				routers,
// 			));
//
// 			assert_ok!(LiquidityPoolsGateway::submit(
// 				sender.clone(),
// 				domain.clone(),
// 				msg.clone()
// 			));
//
// 			let expected_nonce = OutboundMessageNonce::one();
//
// 			let queue_entry = OutboundMessageQueue::<Runtime>::get(expected_nonce)
// 				.expect("an entry is added to the queue");
//
// 			let gateway_sender = <Runtime as Config>::Sender::get();
//
// 			assert_eq!(queue_entry, (domain.clone(), gateway_sender, msg.clone()));
//
// 			event_exists(Event::<Runtime>::OutboundMessageSubmitted {
// 				sender: <Runtime as Config>::Sender::get(),
// 				domain,
// 				message: msg,
// 			});
// 		});
// 	}
//
// 	#[test]
// 	fn local_domain() {
// 		new_test_ext().execute_with(|| {
// 			let domain = Domain::Centrifuge;
// 			let sender = get_test_account_id();
// 			let msg = Message;
//
// 			assert_noop!(
// 				LiquidityPoolsGateway::submit(sender, domain, msg),
// 				Error::<Runtime>::DomainNotSupported
// 			);
// 		});
// 	}
//
// 	#[test]
// 	fn multi_router_not_found() {
// 		new_test_ext().execute_with(|| {
// 			let domain = Domain::EVM(0);
// 			let sender = get_test_account_id();
// 			let msg = Message;
//
// 			assert_noop!(
// 				LiquidityPoolsGateway::submit(sender, domain, msg),
// 				Error::<Runtime>::MultiRouterNotFound
// 			);
// 		});
// 	}
// }
