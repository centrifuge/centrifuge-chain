use cfg_mocks::MessageMock;
use cfg_traits::connectors::Router;
use frame_support::{assert_noop, assert_ok, traits::fungible::Mutate};
use pallet_evm::AddressMapping;
use pallet_xcm_transactor::RemoteTransactInfoWithMaxWeight;
use sp_core::{bounded_vec, crypto::AccountId32, H160, U256};
use sp_runtime::traits::Convert;

use super::mock::*;
use crate::{
	axelar_evm::AxelarEVMRouter, ethereum_xcm::EthereumXCMRouter, DomainRouter, EVMChain,
	EVMDomain, FeeValues, XcmDomain, XcmTransactInfo,
};

mod axelar_evm {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let sender: AccountId32 = rand::random::<[u8; 32]>().into();
			let sender_h160: H160 =
				H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&sender)[0..20]);
			let derived_sender = IdentityAddressMapping::into_account_id(sender_h160);

			Balances::mint_into(&derived_sender.into(), 1_000_000_000_000_000).unwrap();

			let axelar_contract_address = H160::from_low_u64_be(1);
			let connectors_contract_address = H160::from_low_u64_be(2);

			let transaction_call_cost =
				<Runtime as pallet_evm::Config>::config().gas_transaction_call;

			let evm_domain = EVMDomain {
				chain: EVMChain::Ethereum,
				axelar_contract_address,
				connectors_contract_address,
				fee_values: FeeValues {
					value: U256::from(10),
					gas_limit: U256::from(transaction_call_cost + 10_000),
					gas_price: U256::from(10),
				},
			};

			let domain_router = DomainRouter::<Runtime>::AxelarEVM(AxelarEVMRouter::<Runtime> {
				domain: evm_domain,
				_marker: Default::default(),
			});

			let msg = MessageMock::Second;

			assert_ok!(domain_router.send(sender, msg));
		});
	}

	#[test]
	fn insufficient_balance() {
		new_test_ext().execute_with(|| {
			let sender: AccountId32 = rand::random::<[u8; 32]>().into();

			let axelar_contract_address = H160::from_low_u64_be(1);
			let connectors_contract_address = H160::from_low_u64_be(2);

			let evm_domain = EVMDomain {
				chain: EVMChain::Ethereum,
				axelar_contract_address,
				connectors_contract_address,
				fee_values: FeeValues {
					value: U256::from(1),
					gas_limit: U256::from(10),
					gas_price: U256::from(1),
				},
			};

			let domain_router = DomainRouter::<Runtime>::AxelarEVM(AxelarEVMRouter::<Runtime> {
				domain: evm_domain,
				_marker: Default::default(),
			});

			let msg = MessageMock::Second;

			let res = domain_router.send(sender, msg);
			assert_eq!(
				res.err().unwrap(),
				pallet_evm::Error::<Runtime>::BalanceLow.into()
			);
		});
	}
}

mod ethereum_xcm {
	use super::*;

	mod init {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let currency_id = CurrencyId::OtherReserve(1);
				let dest = CurrencyIdToMultiLocation::convert(currency_id.clone()).unwrap();

				let xcm_domain = XcmDomain {
					location: Box::new(dest.clone().into_versioned()),
					ethereum_xcm_transact_call_index: bounded_vec![0],
					contract_address: H160::from_slice(rand::random::<[u8; 20]>().as_slice()),
					fee_currency: currency_id,
					max_gas_limit: 10,
					transact_info: XcmTransactInfo {
						transact_extra_weight: 1.into(),
						max_weight: 100_000_000_000.into(),
						transact_extra_weight_signed: None,
					},
					fee_per_second: 1u128,
				};

				let domain_router =
					DomainRouter::<Runtime>::EthereumXCM(EthereumXCMRouter::<Runtime> {
						xcm_domain: xcm_domain.clone(),
						_marker: Default::default(),
					});

				assert_ok!(domain_router.init());

				let res = pallet_xcm_transactor::TransactInfoWithWeightLimit::<Runtime>::get(
					dest.clone(),
				)
				.unwrap();

				assert_eq!(
					res.transact_extra_weight,
					xcm_domain.transact_info.transact_extra_weight
				);
				assert_eq!(res.max_weight, xcm_domain.transact_info.max_weight);
				assert_eq!(
					res.transact_extra_weight_signed,
					xcm_domain.transact_info.transact_extra_weight_signed
				);

				assert_eq!(
					pallet_xcm_transactor::DestinationAssetFeePerSecond::<Runtime>::get(dest),
					Some(xcm_domain.fee_per_second),
				);
			});
		}
	}

	mod send {
		use super::*;

		#[test]
		fn success() {
			new_test_ext().execute_with(|| {
				let currency_id = CurrencyId::OtherReserve(1);
				let dest = CurrencyIdToMultiLocation::convert(currency_id.clone()).unwrap();

				let xcm_domain = XcmDomain {
					location: Box::new(dest.clone().into_versioned()),
					ethereum_xcm_transact_call_index: bounded_vec![0],
					contract_address: H160::from_slice(rand::random::<[u8; 20]>().as_slice()),
					fee_currency: currency_id,
					max_gas_limit: 10,
					transact_info: XcmTransactInfo {
						transact_extra_weight: 1.into(),
						max_weight: 100_000_000_000.into(),
						transact_extra_weight_signed: None,
					},
					fee_per_second: 1u128,
				};

				let domain_router =
					DomainRouter::<Runtime>::EthereumXCM(EthereumXCMRouter::<Runtime> {
						xcm_domain: xcm_domain.clone(),
						_marker: Default::default(),
					});

				// Manually insert the transact weight info in the `TransactInfoWithWeightLimit`
				// storage.

				pallet_xcm_transactor::TransactInfoWithWeightLimit::<Runtime>::insert(
					dest.clone(),
					RemoteTransactInfoWithMaxWeight {
						transact_extra_weight: xcm_domain
							.transact_info
							.transact_extra_weight
							.clone(),
						max_weight: xcm_domain.transact_info.max_weight.clone(),
						transact_extra_weight_signed: None,
					},
				);

				// Manually insert the fee per second in the `DestinationAssetFeePerSecond`
				// storage.

				pallet_xcm_transactor::DestinationAssetFeePerSecond::<Runtime>::insert(
					dest,
					xcm_domain.fee_per_second.clone(),
				);

				let sender: AccountId32 = rand::random::<[u8; 32]>().into();
				let msg = MessageMock::Second;

				assert_ok!(domain_router.send(sender, msg));
			});
		}

		#[test]
		fn success_with_init() {
			new_test_ext().execute_with(|| {
				let currency_id = CurrencyId::OtherReserve(1);
				let dest = CurrencyIdToMultiLocation::convert(currency_id.clone()).unwrap();

				let xcm_domain = XcmDomain {
					location: Box::new(dest.clone().into_versioned()),
					ethereum_xcm_transact_call_index: bounded_vec![0],
					contract_address: H160::from_slice(rand::random::<[u8; 20]>().as_slice()),
					fee_currency: currency_id,
					max_gas_limit: 10,
					transact_info: XcmTransactInfo {
						transact_extra_weight: 1.into(),
						max_weight: 100_000_000_000.into(),
						transact_extra_weight_signed: None,
					},
					fee_per_second: 1u128,
				};

				let domain_router =
					DomainRouter::<Runtime>::EthereumXCM(EthereumXCMRouter::<Runtime> {
						xcm_domain: xcm_domain.clone(),
						_marker: Default::default(),
					});

				assert_ok!(domain_router.init());

				let sender: AccountId32 = rand::random::<[u8; 32]>().into();
				let msg = MessageMock::Second;

				assert_ok!(domain_router.send(sender, msg));
			});
		}

		#[test]
		fn transactor_info_not_set() {
			new_test_ext().execute_with(|| {
				let currency_id = CurrencyId::OtherReserve(1);
				let dest = CurrencyIdToMultiLocation::convert(currency_id.clone()).unwrap();

				let xcm_domain = XcmDomain {
					location: Box::new(dest.clone().into_versioned()),
					ethereum_xcm_transact_call_index: bounded_vec![0],
					contract_address: H160::from_slice(rand::random::<[u8; 20]>().as_slice()),
					fee_currency: currency_id,
					max_gas_limit: 10,
					transact_info: XcmTransactInfo {
						transact_extra_weight: 1.into(),
						max_weight: 100_000_000_000.into(),
						transact_extra_weight_signed: None,
					},
					fee_per_second: 1u128,
				};

				let domain_router =
					DomainRouter::<Runtime>::EthereumXCM(EthereumXCMRouter::<Runtime> {
						xcm_domain: xcm_domain.clone(),
						_marker: Default::default(),
					});

				// Manually insert the fee per second in the `DestinationAssetFeePerSecond`
				// storage.

				pallet_xcm_transactor::DestinationAssetFeePerSecond::<Runtime>::insert(
					dest,
					xcm_domain.fee_per_second.clone(),
				);

				let sender: AccountId32 = rand::random::<[u8; 32]>().into();
				let msg = MessageMock::Second;

				assert_noop!(
					domain_router.send(sender, msg),
					pallet_xcm_transactor::Error::<Runtime>::TransactorInfoNotSet,
				);
			});
		}

		#[test]
		fn fee_per_second_not_set() {
			new_test_ext().execute_with(|| {
				let currency_id = CurrencyId::OtherReserve(1);
				let dest = CurrencyIdToMultiLocation::convert(currency_id.clone()).unwrap();

				let xcm_domain = XcmDomain {
					location: Box::new(dest.clone().into_versioned()),
					ethereum_xcm_transact_call_index: bounded_vec![0],
					contract_address: H160::from_slice(rand::random::<[u8; 20]>().as_slice()),
					fee_currency: currency_id,
					max_gas_limit: 10,
					transact_info: XcmTransactInfo {
						transact_extra_weight: 1.into(),
						max_weight: 100_000_000_000.into(),
						transact_extra_weight_signed: None,
					},
					fee_per_second: 1u128,
				};

				let domain_router =
					DomainRouter::<Runtime>::EthereumXCM(EthereumXCMRouter::<Runtime> {
						xcm_domain: xcm_domain.clone(),
						_marker: Default::default(),
					});

				// Manually insert the transact weight info in the `TransactInfoWithWeightLimit`
				// storage.

				pallet_xcm_transactor::TransactInfoWithWeightLimit::<Runtime>::insert(
					dest.clone(),
					RemoteTransactInfoWithMaxWeight {
						transact_extra_weight: xcm_domain
							.transact_info
							.transact_extra_weight
							.clone(),
						max_weight: xcm_domain.transact_info.max_weight.clone(),
						transact_extra_weight_signed: None,
					},
				);

				let sender: AccountId32 = rand::random::<[u8; 32]>().into();
				let msg = MessageMock::Second;

				assert_noop!(
					domain_router.send(sender, msg),
					pallet_xcm_transactor::Error::<Runtime>::FeePerSecondNotSet,
				);
			});
		}
	}
}
