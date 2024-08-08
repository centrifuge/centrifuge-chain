use cfg_primitives::CFG;
use cfg_traits::liquidity_pools::Router;
use frame_support::{assert_noop, assert_ok, traits::fungible::Mutate, BoundedVec};
use lazy_static::lazy_static;
use pallet_evm::AddressMapping;
use sp_core::{crypto::AccountId32, H160, H256, U256};
use sp_runtime::{
	traits::{BlakeTwo256, ConstU32, Hash},
	DispatchError,
};

use super::mock::*;
use crate::*;

lazy_static! {
	static ref TEST_EVM_CHAIN: BoundedVec<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>> =
		BoundedVec::<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>::try_from(
			"ethereum".as_bytes().to_vec()
		)
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
					value: U256::from(0),
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
					res.err().unwrap().error,
					pallet_evm::Error::<Runtime>::BalanceLow.into()
				);
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
			pub msg: Vec<u8>,
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
					value: U256::from(0),
					gas_limit: U256::from(10),
					gas_price: U256::from(10),
				},
			};

			let sender: AccountId32 = [0; 32].into();
			let sender_h160: H160 =
				H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&sender)[0..20]);
			let derived_sender = IdentityAddressMapping::into_account_id(sender_h160);

			let msg = vec![0x42];

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

				assert_ok!(Balances::mint_into(
					&test_data.derived_sender.clone(),
					1_000_000 * CFG
				));

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
					});

				let res = domain_router.send(test_data.sender, test_data.msg);

				assert_eq!(
					res.err().unwrap().error,
					pallet_evm::Error::<Runtime>::BalanceLow.into()
				);
			});
		}
	}
}
