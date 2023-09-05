use ::xcm::{
	lts::WeightLimit,
	v2::OriginKind,
	v3::{Instruction::*, MultiAsset, Weight},
};
use cfg_mocks::MessageMock;
use cfg_primitives::CFG;
use cfg_traits::liquidity_pools::{Codec, Router};
use cumulus_primitives_core::MultiLocation;
use frame_support::{assert_noop, assert_ok, traits::fungible::Mutate};
use lazy_static::lazy_static;
use pallet_evm::AddressMapping;
use sp_core::{bounded_vec, crypto::AccountId32, H160, H256, U256};
use sp_runtime::{
	traits::{BlakeTwo256, Convert, Hash},
	DispatchError,
};

use super::mock::*;
use crate::*;

lazy_static! {
	static ref TEST_EVM_CHAIN: BoundedVec<u8, ConstU32<MAX_EVM_CHAIN_SIZE>> =
		BoundedVec::<u8, ConstU32<MAX_EVM_CHAIN_SIZE>>::try_from("ethereum".as_bytes().to_vec())
			.unwrap();
}

mod evm_router {
	use util::*;

	use super::*;

	mod util {
		use super::*;

		pub struct EVMRouterTestData {
			pub test_contract_address: H160,
			pub test_contract_code: Vec<u8>,
			pub test_contract_hash: H256,
			pub evm_domain: EVMDomain,
			pub sender: AccountId32,
			pub sender_h160: H160,
			pub derived_sender: AccountId32,
			pub msg: Vec<u8>,
		}

		pub fn get_test_data() -> EVMRouterTestData {
			let test_contract_address = H160::from_low_u64_be(1);
			let test_contract_code = [0; 32].to_vec();
			let test_contract_hash = BlakeTwo256::hash_of(&test_contract_code);

			let evm_domain = EVMDomain {
				target_contract_address: test_contract_address,
				target_contract_hash: test_contract_hash,
				fee_values: FeeValues {
					value: U256::from(10),
					gas_limit: U256::from(10),
					gas_price: U256::from(10),
				},
			};

			let sender: AccountId32 = [0; 32].into();
			let sender_h160: H160 =
				H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&sender)[0..20]);
			let derived_sender = IdentityAddressMapping::into_account_id(sender_h160);

			let msg = vec![0, 1, 2];

			EVMRouterTestData {
				test_contract_address,
				test_contract_code,
				test_contract_hash,
				evm_domain,
				sender,
				sender_h160,
				derived_sender,
				msg,
			}
		}
	}

	mod init {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				pallet_evm::AccountCodes::<Runtime>::insert(
					test_data.test_contract_address,
					test_data.test_contract_code,
				);

				let router = EVMRouter::<Runtime> {
					evm_domain: test_data.evm_domain,
					_marker: Default::default(),
				};

				assert_ok!(router.do_init());
			});
		}

		#[test]
		fn failure() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let router = EVMRouter::<Runtime> {
					evm_domain: test_data.evm_domain,
					_marker: Default::default(),
				};

				assert_noop!(
					router.do_init(),
					DispatchError::Other("Target contract code does not match")
				);

				pallet_evm::AccountCodes::<Runtime>::insert(
					test_data.test_contract_address,
					[1; 32].to_vec(),
				);

				assert_noop!(
					router.do_init(),
					DispatchError::Other("Target contract code does not match")
				);
			});
		}
	}

	mod send {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let mut test_data = get_test_data();

				Balances::mint_into(&test_data.derived_sender.into(), 1_000_000 * CFG).unwrap();

				let transaction_call_cost =
					<Runtime as pallet_evm::Config>::config().gas_transaction_call;

				test_data.evm_domain.fee_values.gas_limit =
					U256::from(transaction_call_cost + 10_000);

				let router = EVMRouter::<Runtime> {
					evm_domain: test_data.evm_domain,
					_marker: Default::default(),
				};

				assert_ok!(router.do_send(test_data.sender, test_data.msg));
			});
		}

		#[test]
		fn insufficient_balance() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let router = EVMRouter::<Runtime> {
					evm_domain: test_data.evm_domain,
					_marker: Default::default(),
				};

				let res = router.do_send(test_data.sender, test_data.msg);

				assert_eq!(
					res.err().unwrap(),
					pallet_evm::Error::<Runtime>::BalanceLow.into()
				);
			});
		}
	}
}

mod xcm_router {
	use util::*;

	use super::*;

	mod util {
		use super::*;

		pub struct XCMRouterTestData {
			pub currency_id: CurrencyId,
			pub dest: MultiLocation,
			pub xcm_domain: XcmDomain<<Runtime as pallet_xcm_transactor::Config>::CurrencyId>,
			pub sender: AccountId32,
			pub msg: Vec<u8>,
		}

		pub fn get_test_data() -> XCMRouterTestData {
			let currency_id = CurrencyId::OtherReserve(1);
			let dest = CurrencyIdToMultiLocation::convert(currency_id.clone()).unwrap();

			let xcm_domain = XcmDomain {
				location: Box::new(dest.clone().into_versioned()),
				ethereum_xcm_transact_call_index: bounded_vec![0],
				contract_address: H160::from_slice([0; 20].as_slice()),
				max_gas_limit: 10,
				fee_currency: currency_id.clone(),
				fee_per_second: 1u128,
			};

			let sender: AccountId32 = [0; 32].into();

			let msg = vec![0, 1, 2];

			XCMRouterTestData {
				currency_id,
				dest,
				xcm_domain,
				sender,
				msg,
			}
		}
	}

	mod init {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let router = XCMRouter::<Runtime> {
					xcm_domain: test_data.xcm_domain.clone(),
					_marker: Default::default(),
				};

				assert_ok!(router.do_init());
			});
		}
	}

	mod send {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let router = XCMRouter::<Runtime> {
					xcm_domain: test_data.xcm_domain.clone(),
					_marker: Default::default(),
				};

				assert_ok!(router.do_send(test_data.sender, test_data.msg.clone()));

				let sent_messages = sent_xcm();
				assert_eq!(sent_messages.len(), 1);

				let transact_weight = Weight::from_parts(
					test_data.xcm_domain.max_gas_limit * GAS_TO_WEIGHT_MULTIPLIER,
					DEFAULT_PROOF_SIZE.saturating_div(2),
				);

				let overall_weight = Weight::from_parts(
					transact_weight.ref_time() + XCM_INSTRUCTION_WEIGHT * 3,
					DEFAULT_PROOF_SIZE,
				);

				let fees = Into::<u128>::into(overall_weight.ref_time())
					* test_data.xcm_domain.fee_per_second;

				let (_, xcm) = sent_messages.first().unwrap();
				assert!(xcm.0.contains(&WithdrawAsset(
					(MultiAsset {
						id: ::xcm::v3::AssetId::Concrete(MultiLocation::here()),
						fun: ::xcm::v3::Fungibility::Fungible(fees),
					})
					.into()
				)));

				assert!(xcm.0.contains(&BuyExecution {
					fees: MultiAsset {
						id: ::xcm::v3::AssetId::Concrete(MultiLocation::here()),
						fun: ::xcm::v3::Fungibility::Fungible(fees),
					},
					weight_limit: WeightLimit::Limited(overall_weight),
				}));

				let expected_call = get_encoded_ethereum_xcm_call::<Runtime>(
					test_data.xcm_domain.clone(),
					test_data.msg,
				)
				.unwrap();

				assert!(xcm.0.contains(&Transact {
					origin_kind: OriginKind::SovereignAccount,
					require_weight_at_most: transact_weight,
					call: expected_call.into(),
				}));
			});
		}

		#[test]
		fn success_with_init() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let router = XCMRouter::<Runtime> {
					xcm_domain: test_data.xcm_domain.clone(),
					_marker: Default::default(),
				};

				assert_ok!(router.do_init());

				assert_ok!(router.do_send(test_data.sender, test_data.msg));
			});
		}

		#[test]
		fn transactor_info_not_set() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let router = XCMRouter::<Runtime> {
					xcm_domain: test_data.xcm_domain.clone(),
					_marker: Default::default(),
				};

				// Manually insert the fee per second in the `DestinationAssetFeePerSecond`
				// storage.

				pallet_xcm_transactor::DestinationAssetFeePerSecond::<Runtime>::insert(
					test_data.dest,
					test_data.xcm_domain.fee_per_second.clone(),
				);

				// We ensure we can send although no `TransactInfo is set`
				assert_ok!(router.do_send(test_data.sender, test_data.msg),);
			});
		}
	}
}

mod axelar_evm {
	use util::*;

	use super::*;

	mod util {
		use super::*;

		pub struct AxelarEVMTestData {
			pub axelar_contract_address: H160,
			pub axelar_contract_code: Vec<u8>,
			pub axelar_contract_hash: H256,
			pub liquidity_pools_contract_address: H160,
			pub evm_domain: EVMDomain,
			pub sender: AccountId32,
			pub sender_h160: H160,
			pub derived_sender: AccountId32,
			pub msg: MessageMock,
		}

		pub fn get_test_data() -> AxelarEVMTestData {
			let axelar_contract_address = H160::from_low_u64_be(1);
			let axelar_contract_code = [0; 32].to_vec();
			let axelar_contract_hash = BlakeTwo256::hash_of(&axelar_contract_code);
			let liquidity_pools_contract_address = H160::from_low_u64_be(2);

			let evm_domain = EVMDomain {
				target_contract_address: axelar_contract_address,
				target_contract_hash: axelar_contract_hash,
				fee_values: FeeValues {
					value: U256::from(10),
					gas_limit: U256::from(10),
					gas_price: U256::from(10),
				},
			};

			let sender: AccountId32 = [0; 32].into();
			let sender_h160: H160 =
				H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&sender)[0..20]);
			let derived_sender = IdentityAddressMapping::into_account_id(sender_h160);

			let msg = MessageMock::Second;

			AxelarEVMTestData {
				axelar_contract_address,
				axelar_contract_code,
				axelar_contract_hash,
				liquidity_pools_contract_address,
				evm_domain,
				sender,
				sender_h160,
				derived_sender,
				msg,
			}
		}
	}

	mod init {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				pallet_evm::AccountCodes::<Runtime>::insert(
					test_data.axelar_contract_address,
					test_data.axelar_contract_code,
				);

				let domain_router =
					DomainRouter::<Runtime>::AxelarEVM(AxelarEVMRouter::<Runtime> {
						router: EVMRouter {
							evm_domain: test_data.evm_domain,
							_marker: Default::default(),
						},
						evm_chain: TEST_EVM_CHAIN.clone(),
						liquidity_pools_contract_address: test_data
							.liquidity_pools_contract_address,
						_marker: Default::default(),
					});

				assert_ok!(domain_router.init());
			});
		}

		#[test]
		fn failure() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let domain_router =
					DomainRouter::<Runtime>::AxelarEVM(AxelarEVMRouter::<Runtime> {
						router: EVMRouter {
							evm_domain: test_data.evm_domain,
							_marker: Default::default(),
						},
						evm_chain: TEST_EVM_CHAIN.clone(),
						liquidity_pools_contract_address: test_data
							.liquidity_pools_contract_address,
						_marker: Default::default(),
					});

				assert_noop!(
					domain_router.init(),
					DispatchError::Other("Target contract code does not match")
				);

				pallet_evm::AccountCodes::<Runtime>::insert(
					test_data.axelar_contract_address,
					[1; 32].to_vec(),
				);

				assert_noop!(
					domain_router.init(),
					DispatchError::Other("Target contract code does not match")
				);
			});
		}
	}

	mod send {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let mut test_data = get_test_data();

				Balances::mint_into(&test_data.derived_sender.into(), 1_000_000 * CFG).unwrap();

				let transaction_call_cost =
					<Runtime as pallet_evm::Config>::config().gas_transaction_call;

				test_data.evm_domain.fee_values.gas_limit =
					U256::from(transaction_call_cost + 10_000);

				let domain_router =
					DomainRouter::<Runtime>::AxelarEVM(AxelarEVMRouter::<Runtime> {
						router: EVMRouter {
							evm_domain: test_data.evm_domain,
							_marker: Default::default(),
						},
						evm_chain: TEST_EVM_CHAIN.clone(),
						liquidity_pools_contract_address: test_data
							.liquidity_pools_contract_address,
						_marker: Default::default(),
					});

				assert_ok!(domain_router.send(test_data.sender, test_data.msg));
			});
		}

		#[test]
		fn insufficient_balance() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let domain_router =
					DomainRouter::<Runtime>::AxelarEVM(AxelarEVMRouter::<Runtime> {
						router: EVMRouter {
							evm_domain: test_data.evm_domain,
							_marker: Default::default(),
						},
						evm_chain: TEST_EVM_CHAIN.clone(),
						liquidity_pools_contract_address: test_data
							.liquidity_pools_contract_address,
						_marker: Default::default(),
					});

				let res = domain_router.send(test_data.sender, test_data.msg);

				assert_eq!(
					res.err().unwrap(),
					pallet_evm::Error::<Runtime>::BalanceLow.into()
				);
			});
		}
	}
}

mod axelar_xcm {
	use util::*;

	use super::*;

	mod util {
		use super::*;

		pub struct AxelarXCMTestData {
			pub currency_id: CurrencyId,
			pub dest: MultiLocation,
			pub xcm_domain: XcmDomain<<Runtime as pallet_xcm_transactor::Config>::CurrencyId>,
			pub axelar_target_chain: BoundedVec<u8, ConstU32<MAX_EVM_CHAIN_SIZE>>,
			pub axelar_target_contract: H160,
			pub sender: AccountId32,
			pub msg: MessageMock,
		}

		pub fn get_test_data() -> AxelarXCMTestData {
			let currency_id = CurrencyId::OtherReserve(1);
			let dest = CurrencyIdToMultiLocation::convert(currency_id.clone()).unwrap();

			let xcm_domain = XcmDomain {
				location: Box::new(dest.clone().into_versioned()),
				ethereum_xcm_transact_call_index: bounded_vec![0],
				contract_address: H160::from_slice([0; 20].as_slice()),
				max_gas_limit: 10,
				fee_currency: currency_id.clone(),
				fee_per_second: 1u128,
			};
			let axelar_target_chain = TEST_EVM_CHAIN.clone();
			let axelar_target_contract = H160::from_low_u64_be(1);

			let sender: AccountId32 = [0; 32].into();

			let msg = MessageMock::First;

			AxelarXCMTestData {
				currency_id,
				dest,
				xcm_domain,
				axelar_target_chain,
				axelar_target_contract,
				sender,
				msg,
			}
		}
	}

	mod init {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let domain_router =
					DomainRouter::<Runtime>::AxelarXCM(AxelarXCMRouter::<Runtime> {
						router: XCMRouter {
							xcm_domain: test_data.xcm_domain.clone(),
							_marker: Default::default(),
						},
						axelar_target_chain: test_data.axelar_target_chain,
						axelar_target_contract: test_data.axelar_target_contract,
						_marker: Default::default(),
					});

				assert_ok!(domain_router.init());
			});
		}
	}

	mod send {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let domain_router =
					DomainRouter::<Runtime>::AxelarXCM(AxelarXCMRouter::<Runtime> {
						router: XCMRouter {
							xcm_domain: test_data.xcm_domain.clone(),
							_marker: Default::default(),
						},
						axelar_target_chain: test_data.axelar_target_chain.clone(),
						axelar_target_contract: test_data.axelar_target_contract,
						_marker: Default::default(),
					});

				assert_ok!(domain_router.init());

				assert_ok!(domain_router.send(test_data.sender, test_data.msg.clone()));

				let sent_messages = sent_xcm();
				assert_eq!(sent_messages.len(), 1);

				let transact_weight = Weight::from_parts(
					test_data.xcm_domain.max_gas_limit * GAS_TO_WEIGHT_MULTIPLIER,
					DEFAULT_PROOF_SIZE.saturating_div(2),
				);

				let overall_weight = Weight::from_parts(
					transact_weight.ref_time() + XCM_INSTRUCTION_WEIGHT * 3,
					DEFAULT_PROOF_SIZE,
				);

				let fees = Into::<u128>::into(overall_weight.ref_time())
					* test_data.xcm_domain.fee_per_second;

				let (_, xcm) = sent_messages.first().unwrap();
				assert!(xcm.0.contains(&WithdrawAsset(
					(MultiAsset {
						id: ::xcm::v3::AssetId::Concrete(MultiLocation::here()),
						fun: ::xcm::v3::Fungibility::Fungible(fees),
					})
					.into()
				)));

				assert!(xcm.0.contains(&BuyExecution {
					fees: MultiAsset {
						id: ::xcm::v3::AssetId::Concrete(MultiLocation::here()),
						fun: ::xcm::v3::Fungibility::Fungible(fees),
					},
					weight_limit: WeightLimit::Limited(overall_weight),
				}));

				let contract_call = get_axelar_encoded_msg(
					test_data.msg.serialize(),
					test_data.axelar_target_chain.clone().into_inner(),
					test_data.axelar_target_contract,
				)
				.unwrap();

				let expected_call = get_encoded_ethereum_xcm_call::<Runtime>(
					test_data.xcm_domain.clone(),
					contract_call,
				)
				.unwrap();

				assert!(xcm.0.contains(&Transact {
					origin_kind: OriginKind::SovereignAccount,
					require_weight_at_most: transact_weight,
					call: expected_call.into(),
				}));
			});
		}

		#[test]
		fn transactor_info_not_set() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let domain_router =
					DomainRouter::<Runtime>::AxelarXCM(AxelarXCMRouter::<Runtime> {
						router: XCMRouter {
							xcm_domain: test_data.xcm_domain.clone(),
							_marker: Default::default(),
						},
						axelar_target_chain: test_data.axelar_target_chain,
						axelar_target_contract: test_data.axelar_target_contract,
						_marker: Default::default(),
					});

				// Manually insert the fee per second in the `DestinationAssetFeePerSecond`
				// storage.

				pallet_xcm_transactor::DestinationAssetFeePerSecond::<Runtime>::insert(
					test_data.dest,
					test_data.xcm_domain.fee_per_second.clone(),
				);

				// We ensure we can send although no `TransactInfo is set`
				assert_ok!(domain_router.send(test_data.sender, test_data.msg),);
			});
		}
	}
}

mod ethereum_xcm {
	use util::*;

	use super::*;

	mod util {
		use super::*;

		pub struct EthereumXCMTestData {
			pub currency_id: CurrencyId,
			pub dest: MultiLocation,
			pub xcm_domain: XcmDomain<<Runtime as pallet_xcm_transactor::Config>::CurrencyId>,
			pub axelar_target_chain: BoundedVec<u8, ConstU32<MAX_EVM_CHAIN_SIZE>>,
			pub axelar_target_contract: H160,
			pub sender: AccountId32,
			pub msg: MessageMock,
		}

		pub fn get_test_data() -> EthereumXCMTestData {
			let currency_id = CurrencyId::OtherReserve(1);
			let dest = CurrencyIdToMultiLocation::convert(currency_id.clone()).unwrap();

			let xcm_domain = XcmDomain {
				location: Box::new(dest.clone().into_versioned()),
				ethereum_xcm_transact_call_index: bounded_vec![0],
				contract_address: H160::from_slice([0; 20].as_slice()),
				max_gas_limit: 10,
				fee_currency: currency_id.clone(),
				fee_per_second: 1u128,
			};
			let axelar_target_chain = TEST_EVM_CHAIN.clone();
			let axelar_target_contract = H160::from_low_u64_be(1);

			let sender: AccountId32 = [0; 32].into();

			let msg = MessageMock::First;

			EthereumXCMTestData {
				currency_id,
				dest,
				xcm_domain,
				axelar_target_chain,
				axelar_target_contract,
				sender,
				msg,
			}
		}
	}

	mod init {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let domain_router =
					DomainRouter::<Runtime>::EthereumXCM(EthereumXCMRouter::<Runtime> {
						router: XCMRouter {
							xcm_domain: test_data.xcm_domain.clone(),
							_marker: Default::default(),
						},
						_marker: Default::default(),
					});

				assert_ok!(domain_router.init());
			});
		}
	}

	mod send {
		use super::*;

		#[test]
		fn success_with_init() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let domain_router =
					DomainRouter::<Runtime>::EthereumXCM(EthereumXCMRouter::<Runtime> {
						router: XCMRouter {
							xcm_domain: test_data.xcm_domain.clone(),
							_marker: Default::default(),
						},
						_marker: Default::default(),
					});

				assert_ok!(domain_router.init());

				assert_ok!(domain_router.send(test_data.sender, test_data.msg.clone()));

				let sent_messages = sent_xcm();
				assert_eq!(sent_messages.len(), 1);

				let transact_weight = Weight::from_parts(
					test_data.xcm_domain.max_gas_limit * GAS_TO_WEIGHT_MULTIPLIER,
					DEFAULT_PROOF_SIZE.saturating_div(2),
				);

				let overall_weight = Weight::from_parts(
					transact_weight.ref_time() + XCM_INSTRUCTION_WEIGHT * 3,
					DEFAULT_PROOF_SIZE,
				);

				let fees = Into::<u128>::into(overall_weight.ref_time())
					* test_data.xcm_domain.fee_per_second;

				let (_, xcm) = sent_messages.first().unwrap();
				assert!(xcm.0.contains(&WithdrawAsset(
					(MultiAsset {
						id: ::xcm::v3::AssetId::Concrete(MultiLocation::here()),
						fun: ::xcm::v3::Fungibility::Fungible(fees),
					})
					.into()
				)));

				assert!(xcm.0.contains(&BuyExecution {
					fees: MultiAsset {
						id: ::xcm::v3::AssetId::Concrete(MultiLocation::here()),
						fun: ::xcm::v3::Fungibility::Fungible(fees),
					},
					weight_limit: WeightLimit::Limited(overall_weight),
				}));

				let contract_call = get_encoded_contract_call(test_data.msg.serialize()).unwrap();
				let expected_call = get_encoded_ethereum_xcm_call::<Runtime>(
					test_data.xcm_domain.clone(),
					contract_call,
				)
				.unwrap();

				assert!(xcm.0.contains(&Transact {
					origin_kind: OriginKind::SovereignAccount,
					require_weight_at_most: transact_weight,
					call: expected_call.into(),
				}));
			});
		}

		#[test]
		fn transactor_info_not_set() {
			new_test_ext().execute_with(|| {
				let test_data = get_test_data();

				let domain_router =
					DomainRouter::<Runtime>::EthereumXCM(EthereumXCMRouter::<Runtime> {
						router: XCMRouter {
							xcm_domain: test_data.xcm_domain.clone(),
							_marker: Default::default(),
						},
						_marker: Default::default(),
					});

				// Manually insert the fee per second in the `DestinationAssetFeePerSecond`
				// storage.

				pallet_xcm_transactor::DestinationAssetFeePerSecond::<Runtime>::insert(
					test_data.dest,
					test_data.xcm_domain.fee_per_second.clone(),
				);

				// We ensure we can send although no `TransactInfo is set`
				assert_ok!(domain_router.send(test_data.sender, test_data.msg),);
			});
		}
	}
}
