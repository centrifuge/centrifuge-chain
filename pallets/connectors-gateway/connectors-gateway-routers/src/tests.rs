use sp_core::crypto::AccountId32;

use super::mock::*;

mod utils {
	use super::*;

	pub fn get_random_test_account_id() -> AccountId32 {
		rand::random::<[u8; 32]>().into()
	}
}

use utils::*;

mod domain_router {
	use cfg_mocks::MessageMock;
	use cfg_traits::connectors::Router;

	use super::*;
	use crate::{
		axelar_evm::{AxelarEVMRouter, EVMChain, EVMDomain},
		DomainRouter, FeeValues,
	};

	#[test]
	fn axelar_evm_success() {
		new_test_ext().execute_with(|| {
			let evm_domain = EVMDomain {
				chain: EVMChain::Ethereum,
				axelar_contract_address: Default::default(),
				connectors_contract_address: Default::default(),
				fee_values: FeeValues {
					value: Default::default(),
					gas_limit: 0,
					max_fee_per_gas: Default::default(),
					max_priority_fee_per_gas: None,
				},
			};

			let domain_router = DomainRouter::<Runtime>::AxelarEVM(AxelarEVMRouter::<Runtime> {
				domain: evm_domain,
				_phantom: Default::default(),
			});

			let sender = get_random_test_account_id();
			let msg = MessageMock::Second;

			domain_router.send(sender, msg).unwrap()
		});
	}
}
