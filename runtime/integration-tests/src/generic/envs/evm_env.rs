use std::{collections::HashMap, marker::PhantomData};

use ethabi::{ethereum_types, Log, RawLog, Token};
use pallet_evm::{CallInfo, ExitReason, FeeCalculator, Runner};
use sp_core::{H160, U256};
use sp_runtime::DispatchError;

use crate::{
	generic::{
		config::Runtime,
		env, utils,
		utils::{
			evm,
			evm::{ContractInfo, DeployedContractInfo},
			ESSENTIAL,
		},
	},
	utils::accounts::Keyring,
};

const GAS_LIMIT: u64 = 15_000_000;
const VALIDATE: bool = true;
const TRANSACTIONAL: bool = true;

pub struct EvmEnv<T: Runtime> {
	sol_contracts: Option<HashMap<String, ContractInfo>>,
	deployed_contracts: HashMap<String, DeployedContractInfo>,
	_phantom: PhantomData<T>,
}

impl<T: Runtime> Default for EvmEnv<T> {
	fn default() -> Self {
		EvmEnv {
			sol_contracts: None,
			deployed_contracts: HashMap::new(),
			_phantom: Default::default(),
		}
	}
}

impl<T: Runtime> env::EvmEnv<T> for EvmEnv<T> {
	fn find_events(&self, contract: impl Into<String>, event: impl Into<String>) -> Vec<Log> {
		let contract = self.contract(contract).contract;
		let event = contract
			.event(Into::<String>::into(event).as_ref())
			.unwrap();

		pallet_ethereum::Pending::<T>::get()
			.into_iter()
			.map(|(_, status, _)| status.logs)
			.flatten()
			.collect::<Vec<_>>()
			.into_iter()
			.filter_map(|log| {
				event
					.parse_log(RawLog {
						topics: log
							.topics
							.into_iter()
							.map(|h| ethereum_types::H256::from(h.0))
							.collect(),
						data: log.data,
					})
					.ok()
			})
			.collect()
	}

	fn load_contracts(&mut self) -> &mut Self {
		self.sol_contracts = Some(evm::fetch_contracts());
		self
	}

	fn deployed(&self, name: impl Into<String>) -> DeployedContractInfo {
		self.deployed_contracts
			.get(&name.into())
			.expect("Not deployed")
			.clone()
	}

	fn register(
		&mut self,
		name: impl Into<String>,
		contract: impl Into<String>,
		address: Option<H160>,
	) -> &mut Self {
		let contract = self.contract(contract);
		let (address, runtime_code) = if let Some(given) = address {
			let code = pallet_evm::AccountCodes::<T>::get(given);
			assert_eq!(
				code, contract.deployed_bytecode,
				"Can not register contract. Contract bytecode not matching."
			);

			(given, code)
		} else {
			let mut found = pallet_evm::AccountCodes::<T>::iter()
				.filter(|(_, code)| code == &contract.deployed_bytecode)
				.collect::<Vec<_>>();

			assert_eq!(
				found.len(),
				1,
				"Can not register contract. Multiple where found."
			);

			found.pop().expect("Len is one. qed.")
		};

		self.deployed_contracts.insert(
			name.into(),
			DeployedContractInfo::new(
				contract.contract,
				runtime_code,
				ethabi::ethereum_types::H160::from(address.0),
			),
		);
		self
	}

	fn contract(&self, name: impl Into<String>) -> ContractInfo {
		self.sol_contracts
			.as_ref()
			.expect("Need to load_contracts first")
			.get(&name.into())
			.expect("Not loaded")
			.clone()
	}

	fn deploy(
		&mut self,
		what: impl Into<String> + Clone,
		name: impl Into<String>,
		who: Keyring,
		args: Option<&[Token]>,
	) -> &mut Self {
		let info = self
			.sol_contracts
			.as_ref()
			.expect("Need to load_contracts first")
			.get(&what.clone().into())
			.expect("Unknown contract")
			.clone();

		let init = match (info.contract.constructor(), args) {
			(None, None) => info.bytecode.to_vec(),
			(Some(constructor), Some(args)) => constructor
				.encode_input(info.bytecode.to_vec(), args)
				.expect("Could not encode constructor and arguments."),
			(Some(constructor), None) => constructor
				.encode_input(info.bytecode.to_vec(), &[])
				.expect("Could not encode constructor and argument."),
			(None, Some(_)) => panic!("Contract has no constructor."),
		};

		let create_info = {
			let (base_fee, _) = <T as pallet_evm::Config>::FeeCalculator::min_gas_price();

			<T as pallet_evm::Config>::Runner::create(
				who.into(),
				init,
				0u8.into(),
				10 * GAS_LIMIT,
				Some(base_fee),
				None,
				None,
				Vec::new(),
				false,
				false,
				None,
				None,
				// NOTE: We are using a non standard config here to be able to use large contracts
				//       that are foundry based deployers
				&evm::deployment_config(),
			)
			.expect("Contract creation failed.")
		};

		assert!(
			matches!(create_info.exit_reason, ExitReason::Succeed(_)),
			"{}",
			format!("Contract creation failed: {:?}", create_info.exit_reason)
		);

		self.register(name, what, Some(create_info.value))
	}

	fn call(
		&self,
		caller: Keyring,
		value: U256,
		contract: impl Into<String>,
		function: impl Into<String>,
		args: Option<&[Token]>,
	) -> Result<CallInfo, DispatchError> {
		let contract_info = self
			.deployed_contracts
			.get(&contract.into())
			.expect("Contract not deployed")
			.clone();
		let input = contract_info
			.contract
			.functions_by_name(function.into().as_ref())
			.expect(ESSENTIAL)
			.iter()
			.filter_map(|f| f.encode_input(args.unwrap_or_default()).ok())
			.collect::<Vec<_>>()
			.pop()
			.expect("No matching function Signature found.");

		let (base_fee, _) = <T as pallet_evm::Config>::FeeCalculator::min_gas_price();

		let res = <T as pallet_evm::Config>::Runner::call(
			caller.into(),
			sp_core::H160::from(contract_info.address().0),
			input,
			value,
			GAS_LIMIT,
			Some(base_fee),
			None,
			None,
			Vec::new(),
			// NOTE: Taken from pallet-evm implementation
			TRANSACTIONAL,
			// NOTE: Taken from pallet-evm implementation
			VALIDATE,
			None,
			None,
			<T as pallet_evm::Config>::config(),
		)
		.map_err(|re| re.error)?;

		match res.exit_reason {
			ExitReason::Succeed(_) => Ok(res),
			ExitReason::Fatal(_) => Err(DispatchError::Other("EVM call failed: Fatal")),
			ExitReason::Error(_) => Err(DispatchError::Other("EVM call failed: Error")),
			ExitReason::Revert(_) => Err(DispatchError::Other("EVM call failed: Revert")),
		}
	}

	fn view(
		&self,
		caller: Keyring,
		contract: impl Into<String>,
		function: impl Into<String>,
		args: Option<&[Token]>,
	) -> Result<CallInfo, DispatchError> {
		self.call(caller, U256::zero(), contract, function, args)
	}
}
