use cfg_traits::{
	ethereum::EthereumTransactor,
	liquidity_pools::{MessageReceiver, MessageSender},
	PreConditions,
};
use cfg_types::{domain_address::DomainAddress, EVMChainId};
use ethabi::{Contract, Function, Param, ParamType, Token};
use fp_evm::{ExitError, PrecompileHandle};
use frame_support::{
	pallet_prelude::*,
	weights::{constants::RocksDbWeight, Weight},
	BoundedVec,
};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use precompile_utils::prelude::*;
use sp_core::{H160, H256, U256};
use sp_std::collections::btree_map::BTreeMap;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// Maximum size allowed for a byte representation of an Axelar EVM chain
/// string, as found below:
/// <https://docs.axelar.dev/dev/reference/mainnet-chain-names>
/// <https://docs.axelar.dev/dev/reference/testnet-chain-names>
const MAX_AXELAR_EVM_CHAIN_SIZE: u32 = 16;

const MAX_SOURCE_CHAIN_BYTES: u32 = 128;
// Ensure we allow enough to support a hex encoded address with the `0x` prefix.
const MAX_SOURCE_ADDRESS_BYTES: u32 = 42;
const MAX_TOKEN_SYMBOL_BYTES: u32 = 32;
const MAX_PAYLOAD_BYTES: u32 = 1024;
const EVM_ADDRESS_LEN: usize = 20;

pub type ChainName = BoundedVec<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>;

/// Type to represent the kind of message received by Axelar
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum AxelarId {
	Evm(EVMChainId),
}

/// Configuration for outbound messages though axelar
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AxelarConfig {
	/// Address of liquidity pool contract in the target chain
	pub liquidity_pools_contract_address: H160,

	/// Configuration for executing the EVM call.
	pub domain: DomainConfig,
}

/// Specific domain configuration
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum DomainConfig {
	Evm(EvmConfig),
}

/// Data for validating and executing the internal EVM call.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EvmConfig {
	/// Associated chain id
	pub chain_id: EVMChainId,

	/// The address of the contract deployed in our EVM.
	pub target_contract_address: H160,

	/// The `BlakeTwo256` hash of the target contract code.
	///
	/// This is used during router initialization to ensure that the correct
	/// contract code is used.
	pub target_contract_hash: H256,

	/// The values used when executing the EVM call.
	pub fee_values: FeeValues,
}

/// The FeeValues holds all information related to the transaction costs.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct FeeValues {
	/// The value used when executing the EVM call.
	pub value: U256,

	/// The gas price used when executing the EVM call.
	pub gas_price: U256,

	/// The gas limit used when executing the EVM call.
	pub gas_limit: U256,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The origin that is allowed to set the gateway address we accept
		/// messages from
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The target of the messages comming from other chains
		type Receiver: MessageReceiver<Middleware = Self::Middleware, Origin = DomainAddress>;

		/// Middleware used by the gateway
		type Middleware: From<AxelarId>;

		/// The target of the messages comming from this chain
		type Transactor: EthereumTransactor;

		/// Checker to ensure an evm account code is registered
		type EvmAccountCodeChecker: PreConditions<(H160, H256), Result = bool>;
	}

	#[pallet::storage]
	pub type Configuration<T: Config> = StorageMap<_, Twox64Concat, ChainName, AxelarConfig>;

	#[pallet::storage]
	pub type ChainNameById<T: Config> = StorageMap<_, Twox64Concat, AxelarId, ChainName>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ConfigSet {
			name: ChainName,
			config: AxelarConfig,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Domain not found.
		DomainNotFound,

		/// Router not found.
		RouterNotFound,

		/// Emit when the evm account code is not registered
		ContractCodeMismatch,
	}

	impl<T: Config> Pallet<T> {
		fn weight_for_set_method() -> Weight {
			Weight::from_parts(50_000_000, 512).saturating_add(RocksDbWeight::get().writes(2))
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(Pallet::<T>::weight_for_set_method())]
		#[pallet::call_index(0)]
		pub fn set_config(
			origin: OriginFor<T>,
			chain_name: ChainName,
			config: AxelarConfig,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			match &config.domain {
				DomainConfig::Evm(evm_config) => {
					ensure!(
						T::EvmAccountCodeChecker::check((
							evm_config.target_contract_address,
							evm_config.target_contract_hash,
						)),
						Error::<T>::ContractCodeMismatch
					);

					ChainNameById::<T>::insert(
						AxelarId::Evm(evm_config.chain_id),
						chain_name.clone(),
					);
				}
			}

			Configuration::<T>::insert(chain_name.clone(), config.clone());

			Self::deposit_event(Event::<T>::ConfigSet {
				name: chain_name,
				config: config,
			});

			Ok(())
		}
	}

	#[precompile_utils::precompile]
	impl<T: Config> Pallet<T> {
		// Mimics:
		//
		//   function execute(
		//         bytes32 commandId,
		//         string calldata sourceChain,
		//         string calldata sourceAddress,
		//         bytes calldata payload
		//     ) external { bytes32 payloadHash = keccak256(payload);
		// 		 if (
		//           !gateway.validateContractCall(
		//              commandId,
		//              sourceChain,
		//              sourceAddress,
		//              payloadHash)
		//           ) revert NotApprovedByGateway();
		//
		//        _execute(sourceChain, sourceAddress, payload);
		// }
		#[precompile::public("execute(bytes32,string,string,bytes)")]
		fn execute(
			handle: &mut impl PrecompileHandle,
			_command_id: H256,
			source_chain: BoundedString<ConstU32<MAX_SOURCE_CHAIN_BYTES>>,
			source_address: BoundedString<ConstU32<MAX_SOURCE_ADDRESS_BYTES>>,
			payload: BoundedBytes<ConstU32<MAX_PAYLOAD_BYTES>>,
		) -> EvmResult {
			let chain_name: ChainName = source_chain
				.as_bytes()
				.to_vec()
				.try_into()
				.map_err(|_| ExitError::Other("unexpected chain id".into()))?;

			let config = Configuration::<T>::get(chain_name)
				.ok_or(ExitError::Other("invalid source chain".into()))?;

			ensure!(
				handle.context().caller == config.liquidity_pools_contract_address,
				ExitError::Other("gateway contract address mismatch".into()),
			);

			match config.domain {
				DomainConfig::Evm(EvmConfig { chain_id, .. }) => {
					let source_address_bytes =
						cfg_utils::decode_var_source::<EVM_ADDRESS_LEN>(source_address.as_bytes())
							.ok_or(ExitError::Other("invalid source address".into()))?;

					let origin = DomainAddress::EVM(chain_id, source_address_bytes);
					let message = payload.as_bytes().to_vec();

					T::Receiver::receive(AxelarId::Evm(chain_id).into(), origin, message)
						.map_err(|e| TryDispatchError::Substrate(e).into())
				}
			}
		}

		// Mimics:
		//
		//     function executeWithToken(
		//         bytes32 commandId,
		//         string calldata sourceChain,
		//         string calldata sourceAddress,
		//         bytes calldata payload,
		//         string calldata tokenSymbol,
		//         uint256 amount
		//     ) external { ...
		//     }
		//
		// Note: NOT SUPPORTED
		//
		#[precompile::public("executeWithToken(bytes32,string,string,bytes,string,uint256)")]
		fn execute_with_token(
			_handle: &mut impl PrecompileHandle,
			_command_id: H256,
			_source_chain: BoundedString<ConstU32<MAX_SOURCE_CHAIN_BYTES>>,
			_source_address: BoundedString<ConstU32<MAX_SOURCE_ADDRESS_BYTES>>,
			_payload: BoundedBytes<ConstU32<MAX_PAYLOAD_BYTES>>,
			_token_symbol: BoundedString<ConstU32<MAX_TOKEN_SYMBOL_BYTES>>,
			_amount: U256,
		) -> EvmResult {
			// TODO: Check whether this is enough or if we should error out
			Ok(())
		}
	}

	impl<T: Config> MessageSender for Pallet<T> {
		type Middleware = AxelarId;
		type Origin = DomainAddress;

		fn send(axelar_id: AxelarId, origin: Self::Origin, message: Vec<u8>) -> DispatchResult {
			let chain_name =
				ChainNameById::<T>::get(axelar_id).ok_or(Error::<T>::DomainNotFound)?;
			let config = Configuration::<T>::get(&chain_name).ok_or(Error::<T>::RouterNotFound)?;

			match config.domain {
				DomainConfig::Evm(evm_config) => {
					let sender_evm_address = H160::from_slice(&origin.address()[0..20]);

					let message = wrap_into_axelar_msg(
						message,
						chain_name.into_inner(),
						config.liquidity_pools_contract_address,
					)
					.map_err(DispatchError::Other)?;

					T::Transactor::call(
						sender_evm_address,
						evm_config.target_contract_address,
						message.as_slice(),
						evm_config.fee_values.value,
						evm_config.fee_values.gas_price,
						evm_config.fee_values.gas_limit,
					)
					.map(|_| ())
					.map_err(|e| e.error)
				}
			}
		}
	}
}

/// Encodes the provided message into the format required for submitting it
/// to the Axelar contract which in turn calls the LiquidityPools
/// contract with the serialized LP message as `payload`.
///
/// Axelar contract call:
/// <https://github.com/axelarnetwork/axelar-cgp-solidity/blob/v4.3.2/contracts/AxelarGateway.sol#L78>
///
/// LiquidityPools contract call:
/// <https://github.com/centrifuge/liquidity-pools/blob/383d279f809a01ab979faf45f31bf9dc3ce6a74a/src/routers/Gateway.sol#L276>
fn wrap_into_axelar_msg(
	serialized_msg: Vec<u8>,
	target_chain: Vec<u8>,
	target_contract: H160,
) -> Result<Vec<u8>, &'static str> {
	const AXELAR_FUNCTION_NAME: &str = "callContract";
	const AXELAR_DESTINATION_CHAIN_PARAM: &str = "destinationChain";
	const AXELAR_DESTINATION_CONTRACT_ADDRESS_PARAM: &str = "destinationContractAddress";
	const AXELAR_PAYLOAD_PARAM: &str = "payload";

	#[allow(deprecated)]
	let encoded_axelar_contract = Contract {
		constructor: None,
		functions: BTreeMap::<String, Vec<Function>>::from([(
			AXELAR_FUNCTION_NAME.into(),
			vec![Function {
				name: AXELAR_FUNCTION_NAME.into(),
				inputs: vec![
					Param {
						name: AXELAR_DESTINATION_CHAIN_PARAM.into(),
						kind: ParamType::String,
						internal_type: None,
					},
					Param {
						name: AXELAR_DESTINATION_CONTRACT_ADDRESS_PARAM.into(),
						kind: ParamType::String,
						internal_type: None,
					},
					Param {
						name: AXELAR_PAYLOAD_PARAM.into(),
						kind: ParamType::Bytes,
						internal_type: None,
					},
				],
				outputs: vec![],
				constant: Some(false),
				state_mutability: Default::default(),
			}],
		)]),
		events: Default::default(),
		errors: Default::default(),
		receive: false,
		fallback: false,
	}
	.function(AXELAR_FUNCTION_NAME)
	.map_err(|_| "cannot retrieve Axelar contract function")?
	.encode_input(&[
		Token::String(
			String::from_utf8(target_chain).map_err(|_| "target chain conversion error")?,
		),
		// Ensure that the target contract is correctly converted to hex.
		//
		// The `to_string` method on the H160 is returning a string containing an ellipsis, such
		// as: 0x1234…7890
		Token::String(format!("0x{}", hex::encode(target_contract.0))),
		Token::Bytes(serialized_msg),
	])
	.map_err(|_| "cannot encode input for Axelar contract function")?;

	Ok(encoded_axelar_contract)
}
