use std::{cell::RefCell, collections::HashMap, marker::PhantomData, mem, rc::Rc};

use cfg_primitives::{AuraId, Balance, BlockNumber, Header};
use cfg_types::ParaId;
use cumulus_primitives_core::PersistedValidationData;
use cumulus_primitives_parachain_inherent::ParachainInherentData;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use ethabi::{ethereum_types, Log, RawLog, Token};
use frame_support::{
	dispatch::GetDispatchInfo,
	inherent::{InherentData, ProvideInherent},
	traits::GenesisBuild,
};
use pallet_evm::{CallInfo, FeeCalculator, Runner};
use parity_scale_codec::Encode;
use sp_api::runtime_decl_for_core::CoreV4;
use sp_block_builder::runtime_decl_for_block_builder::BlockBuilderV6;
use sp_consensus_aura::{Slot, AURA_ENGINE_ID};
use sp_core::{sr25519::Public, H256, U256};
use sp_runtime::{
	traits::Extrinsic,
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	Digest, DigestItem, DispatchError, Storage,
};
use sp_timestamp::Timestamp;

use crate::{
	generic::{
		config::Runtime,
		env::{utils, Env, EvmEnv},
		utils::{
			evm,
			evm::{ContractInfo, DeployedContractInfo},
			ESSENTIAL,
		},
	},
	utils::accounts::Keyring,
};

/// Environment that interact directly with the runtime,
/// without the usage of a client.
pub struct RuntimeEnv<T: Runtime> {
	parachain_ext: Rc<RefCell<sp_io::TestExternalities>>,
	sibling_ext: Rc<RefCell<sp_io::TestExternalities>>,
	pending_extrinsics: Vec<(Keyring, T::RuntimeCallExt)>,
	pending_xcm: Vec<(ParaId, Vec<u8>)>,
	sol_contracts: Option<HashMap<String, ContractInfo>>,
	deployed_contracts: HashMap<String, DeployedContractInfo>,
	_config: PhantomData<T>,
}

impl<T: Runtime> Default for RuntimeEnv<T> {
	fn default() -> Self {
		Self::from_storage(Default::default(), Default::default(), Default::default())
	}
}

const GAS_LIMIT: u64 = 5_000_000;
const VALIDATE: bool = true;
const TRANSACTIONAL: bool = true;

impl<T: Runtime> EvmEnv<T> for RuntimeEnv<T> {
	fn find_events(&self, contract: impl Into<String>, event: impl Into<String>) -> Vec<Log> {
		let contract = self.contract(contract).contract;
		let event = contract
			.event(Into::<String>::into(event).as_ref())
			.unwrap();

		self.parachain_state(|| {
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
		})
	}

	fn load_contracts(mut self) -> Self {
		self.sol_contracts = Some(evm::fetch_contracts());
		self
	}

	fn deployed(&self, name: impl Into<String>) -> DeployedContractInfo {
		self.deployed_contracts
			.get(&name.into())
			.expect("Not deployed")
			.clone()
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
		what: impl Into<String>,
		name: impl Into<String>,
		who: Keyring,
		args: Option<&[Token]>,
	) {
		let info = self
			.sol_contracts
			.as_ref()
			.expect("Need to load_contracts first")
			.get(&what.into())
			.expect("Unknown contract")
			.clone();

		let init = match (info.contract.constructor(), args) {
			(None, None) => info.bytecode.to_vec(),
			(Some(constructor), Some(args)) => constructor
				.encode_input(info.bytecode.to_vec(), args)
				.expect(ESSENTIAL),
			(Some(constructor), None) => constructor
				.encode_input(info.bytecode.to_vec(), &[])
				.expect(ESSENTIAL),
			(None, Some(_)) => panic!("Contract expects constructor arguments."),
		};

		let create_info = self.parachain_state_mut(|| {
			let (base_fee, _) = <T as pallet_evm::Config>::FeeCalculator::min_gas_price();

			<T as pallet_evm::Config>::Runner::create(
				who.into(),
				init,
				0u8.into(),
				GAS_LIMIT,
				Some(base_fee),
				None,
				None,
				Vec::new(),
				// NOTE: Taken from pallet-evm implementation
				VALIDATE,
				// NOTE: Taken from pallet-evm implementation
				TRANSACTIONAL,
				None,
				None,
				<T as pallet_evm::Config>::config(),
			)
			.expect(ESSENTIAL)
		});

		let runtime_code =
			self.parachain_state(|| pallet_evm::AccountCodes::<T>::get(create_info.value));

		// assert_eq!(runtime_code, info.deployed_bytecode);

		self.deployed_contracts.insert(
			name.into(),
			DeployedContractInfo::new(info.contract.clone(), runtime_code, create_info),
		);
	}

	fn call(
		// TODO: Needs to imutable actually, but the current state implementation does
		//       not rollback but error out upon changes, which is not ideal if you want to
		//       test stuff without altering your state.
		&mut self,
		caller: Keyring,
		value: U256,
		contract: impl Into<String>,
		function: impl Into<String>,
		args: Option<&[Token]>,
	) -> Result<CallInfo, DispatchError> {
		self.call(caller, value, contract, function, args)
	}

	fn call_mut(
		&mut self,
		caller: Keyring,
		value: U256,
		contract: impl Into<String>,
		function: impl Into<String>,
		args: Option<&[Token]>,
	) -> Result<CallInfo, DispatchError> {
		self.call_mut(caller, value, contract, function, args)
	}

	fn view(
		&mut self,
		caller: Keyring,
		contract: impl Into<String>,
		function: impl Into<String>,
		args: Option<&[Token]>,
	) -> Result<CallInfo, DispatchError> {
		self.call(caller, U256::zero(), contract, function, args)
	}
}

impl<T: Runtime> RuntimeEnv<T> {
	pub fn call_mut(
		&mut self,
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

		self.parachain_state_mut(|| {
			let (base_fee, _) = <T as pallet_evm::Config>::FeeCalculator::min_gas_price();

			<T as pallet_evm::Config>::Runner::call(
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
				VALIDATE,
				// NOTE: Taken from pallet-evm implementation
				TRANSACTIONAL,
				None,
				None,
				<T as pallet_evm::Config>::config(),
			)
			.map_err(|re| re.error.into())
		})
	}

	pub fn call(
		// TODO: Needs to imutable actually, but the current state implementation does
		//       not rollback but error out upon changes, which is not ideal if you want to
		//       test stuff without altering your state.
		&mut self,
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
			.function(function.into().as_ref())
			.expect(ESSENTIAL)
			.encode_input(args.unwrap_or_default())
			.expect(ESSENTIAL);

		// TODO: Needs to imutable actually, but the current state implementation does
		//       not rollback but error out upon changes, which is not ideal if you want
		// to       test stuff without altering your state.
		self.parachain_state_mut(|| {
			let (base_fee, _) = <T as pallet_evm::Config>::FeeCalculator::min_gas_price();

			<T as pallet_evm::Config>::Runner::call(
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
				VALIDATE,
				// NOTE: Taken from pallet-evm implementation
				TRANSACTIONAL,
				None,
				None,
				<T as pallet_evm::Config>::config(),
			)
			.map_err(|re| re.error.into())
		})
	}
}

impl<T: Runtime> Env<T> for RuntimeEnv<T> {
	fn from_parachain_storage(parachain_storage: Storage) -> Self {
		Self::from_storage(Default::default(), parachain_storage, Default::default())
	}

	fn from_storage(
		mut _relay_storage: Storage,
		mut parachain_storage: Storage,
		mut sibling_storage: Storage,
	) -> Self {
		// Needed for the aura usage
		pallet_aura::GenesisConfig::<T> {
			authorities: vec![AuraId::from(Public([0; 32]))],
		}
		.assimilate_storage(&mut parachain_storage)
		.unwrap();

		let mut parachain_ext = sp_io::TestExternalities::new(parachain_storage);

		parachain_ext.execute_with(|| Self::prepare_block(1));

		// Needed for the aura usage
		pallet_aura::GenesisConfig::<T> {
			authorities: vec![AuraId::from(Public([0; 32]))],
		}
		.assimilate_storage(&mut sibling_storage)
		.unwrap();

		let mut sibling_ext = sp_io::TestExternalities::new(sibling_storage);

		sibling_ext.execute_with(|| Self::prepare_block(1));

		Self {
			parachain_ext: Rc::new(RefCell::new(parachain_ext)),
			sibling_ext: Rc::new(RefCell::new(sibling_ext)),
			pending_extrinsics: Vec::default(),
			pending_xcm: Vec::default(),
			sol_contracts: None,
			deployed_contracts: HashMap::new(),
			_config: PhantomData,
		}
	}

	fn submit_now(
		&mut self,
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
	) -> Result<Balance, DispatchError> {
		let call: T::RuntimeCallExt = call.into();
		let info = call.get_dispatch_info();

		let extrinsic = self.parachain_state(|| {
			let nonce = frame_system::Pallet::<T>::account(who.id()).nonce;
			utils::create_extrinsic::<T>(who, call, nonce)
		});
		let len = extrinsic.encoded_size();

		self.parachain_state_mut(|| {
			let res = T::Api::apply_extrinsic(extrinsic);
			// NOTE: This is our custom error that we are having in the
			//       `PreBalanceTransferExtension` SignedExtension, so we need to
			//        catch that here.
			if let Err(TransactionValidityError::Invalid(InvalidTransaction::Custom(255))) = res {
				Ok(Ok(()))
			} else {
				res
			}
			.unwrap()
		})?;

		let fee = self
			.find_event(|e| match e {
				pallet_transaction_payment::Event::TransactionFeePaid { actual_fee, .. } => {
					Some(actual_fee)
				}
				_ => None,
			})
			.unwrap_or_else(|| {
				self.parachain_state(|| {
					pallet_transaction_payment::Pallet::<T>::compute_fee(len as u32, &info, 0)
				})
			});

		Ok(fee)
	}

	fn submit_later(
		&mut self,
		who: Keyring,
		call: impl Into<T::RuntimeCallExt>,
	) -> Result<(), Box<dyn std::error::Error>> {
		self.pending_extrinsics.push((who, call.into()));
		Ok(())
	}

	fn relay_state_mut<R>(&mut self, _f: impl FnOnce() -> R) -> R {
		unimplemented!("Mutable relay state not implemented for RuntimeEnv")
	}

	fn relay_state<R>(&self, _f: impl FnOnce() -> R) -> R {
		unimplemented!("Relay state not implemented for RuntimeEnv")
	}

	fn parachain_state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R {
		self.parachain_ext.borrow_mut().execute_with(f)
	}

	fn parachain_state<R>(&self, f: impl FnOnce() -> R) -> R {
		self.parachain_ext.borrow_mut().execute_with(|| {
			let version = frame_support::StateVersion::V1;
			let hash = frame_support::storage_root(version);

			let result = f();

			assert_eq!(hash, frame_support::storage_root(version));
			result
		})
	}

	fn sibling_state_mut<R>(&mut self, f: impl FnOnce() -> R) -> R {
		self.sibling_ext.borrow_mut().execute_with(f)
	}

	fn sibling_state<R>(&self, f: impl FnOnce() -> R) -> R {
		self.sibling_ext.borrow_mut().execute_with(|| {
			let version = frame_support::StateVersion::V1;
			let hash = frame_support::storage_root(version);

			let result = f();

			assert_eq!(hash, frame_support::storage_root(version));
			result
		})
	}

	fn __priv_build_block(&mut self, i: BlockNumber) {
		self.process_pending_extrinsics();
		self.parachain_state_mut(|| {
			T::Api::finalize_block();
			Self::prepare_block(i);
		});
	}
}

impl<T: Runtime> RuntimeEnv<T> {
	fn process_pending_extrinsics(&mut self) {
		let pending_extrinsics = mem::replace(&mut self.pending_extrinsics, Vec::default());

		for (who, call) in pending_extrinsics {
			let extrinsic = self.parachain_state(|| {
				let nonce = frame_system::Pallet::<T>::account(who.id()).nonce;
				utils::create_extrinsic::<T>(who, call, nonce)
			});

			self.parachain_state_mut(|| T::Api::apply_extrinsic(extrinsic).unwrap().unwrap());
		}
	}

	pub fn prepare_block(i: BlockNumber) {
		let slot = Slot::from(i as u64);
		let digest = Digest {
			logs: vec![DigestItem::PreRuntime(AURA_ENGINE_ID, slot.encode())],
		};

		let header = Header {
			number: i,
			digest,
			state_root: H256::default(),
			extrinsics_root: H256::default(),
			parent_hash: H256::default(),
		};

		T::Api::initialize_block(&header);

		let timestamp = i as u64 * pallet_aura::Pallet::<T>::slot_duration();
		let inherent_extrinsics = vec![
			Extrinsic::new(Self::cumulus_inherent(i), None).unwrap(),
			Extrinsic::new(Self::timestamp_inherent(timestamp), None).unwrap(),
		];

		for extrinsic in inherent_extrinsics {
			T::Api::apply_extrinsic(extrinsic).unwrap().unwrap();
		}
	}

	fn cumulus_inherent(i: BlockNumber) -> T::RuntimeCallExt {
		let mut inherent_data = InherentData::default();

		let sproof_builder = RelayStateSproofBuilder::default();
		let (relay_parent_storage_root, relay_chain_state) =
			sproof_builder.into_state_root_and_proof();

		let cumulus_inherent = ParachainInherentData {
			validation_data: PersistedValidationData {
				parent_head: vec![].into(),
				relay_parent_number: i,
				max_pov_size: Default::default(),
				relay_parent_storage_root,
			},
			relay_chain_state,
			downward_messages: Default::default(),
			horizontal_messages: Default::default(),
		};

		inherent_data
			.put_data(
				cumulus_primitives_parachain_inherent::INHERENT_IDENTIFIER,
				&cumulus_inherent,
			)
			.unwrap();

		cumulus_pallet_parachain_system::Pallet::<T>::create_inherent(&inherent_data)
			.unwrap()
			.into()
	}

	fn timestamp_inherent(timestamp: u64) -> T::RuntimeCallExt {
		let mut inherent_data = InherentData::default();

		let timestamp_inherent = Timestamp::new(timestamp);

		inherent_data
			.put_data(sp_timestamp::INHERENT_IDENTIFIER, &timestamp_inherent)
			.unwrap();

		pallet_timestamp::Pallet::<T>::create_inherent(&inherent_data)
			.unwrap()
			.into()
	}
}

mod tests {
	use cfg_primitives::CFG;

	use super::*;
	use crate::generic::{env::Blocks, utils::genesis::Genesis};

	fn correct_nonce_for_submit_now<T: Runtime>() {
		let mut env = RuntimeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(pallet_balances::GenesisConfig::<T> {
					balances: vec![(Keyring::Alice.id(), 1 * CFG)],
				})
				.storage(),
		);

		env.submit_now(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();

		env.submit_now(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();
	}

	fn correct_nonce_for_submit_later<T: Runtime>() {
		let mut env = RuntimeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(pallet_balances::GenesisConfig::<T> {
					balances: vec![(Keyring::Alice.id(), 1 * CFG)],
				})
				.storage(),
		);

		env.submit_later(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();

		env.submit_later(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();

		env.pass(Blocks::ByNumber(1));

		env.submit_later(
			Keyring::Alice,
			frame_system::Call::remark { remark: vec![] },
		)
		.unwrap();
	}

	crate::test_for_runtimes!(all, correct_nonce_for_submit_now);
	crate::test_for_runtimes!(all, correct_nonce_for_submit_later);
}
