use cfg_traits::{ethereum::EthereumTransactor, liquidity_pools::MessageSender, PreConditions};
use cfg_types::EVMChainId;
use ethabi::{Contract, Function, Param, ParamType, Token};
use frame_support::{
	pallet_prelude::*,
	weights::{constants::RocksDbWeight, Weight},
	BoundedVec,
};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use sp_core::{H160, H256, U256};
use sp_std::collections::btree_map::BTreeMap;

/// Maximum size allowed for a byte representation of an Axelar EVM chain
/// string, as found below:
/// <https://docs.axelar.dev/dev/reference/mainnet-chain-names>
/// <https://docs.axelar.dev/dev/reference/testnet-chain-names>
const MAX_AXELAR_EVM_CHAIN_SIZE: u32 = 16;

/// Configuration for outbound messages though axelar
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AxelarConfig {
	/// Chain the router will reach by Axelar
	pub target_evm_chain: BoundedVec<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>,

	/// Address of liquidity pool contract in the target chain
	pub liquidity_pools_contract_address: H160,

	/// Configuration for executing the EVM call.
	pub evm: EvmConfig,
}

/// Data for validating and executing the internal EVM call.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EvmConfig {
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

		/// The target of the messages comming from this chain
		type Transactor: EthereumTransactor;

		/// Checker to ensure an evm account code is registered
		type EvmAccountCodeChecker: PreConditions<(H160, H256), Result = bool>;
	}

	#[pallet::storage]
	pub type Configuration<T: Config> = StorageMap<_, Twox64Concat, EVMChainId, AxelarConfig>;

	//#[pallet::storage]
	//pub type Configuration<T: Config> = StorageMap<_, Twox64Concat, H256,
	// EVMChainId>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ConfigSet { config: AxelarConfig },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The given domain is not yet allowlisted, as we have no converter yet
		NoConverterForSource,

		/// A given domain expects a given structure for account bytes and it
		/// was not given here.
		AccountBytesMismatchForDomain,

		/// Router not found.
		RouterNotFound,

		/// Emit when the evm account code is not registered
		ContractCodeNotMatch,
	}

	impl<T: Config> Pallet<T> {
		fn weight_for_set_method() -> Weight {
			Weight::from_parts(50_000_000, 512).saturating_add(RocksDbWeight::get().writes(1))
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(Pallet::<T>::weight_for_set_method())]
		#[pallet::call_index(1)]
		pub fn set_outbound_config(
			origin: OriginFor<T>,
			chain_id: EVMChainId,
			config: AxelarConfig,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			ensure!(
				T::EvmAccountCodeChecker::check((
					config.evm.target_contract_address,
					config.evm.target_contract_hash,
				)),
				Error::<T>::ContractCodeNotMatch
			);

			Configuration::<T>::insert(chain_id, config.clone());

			Self::deposit_event(Event::<T>::ConfigSet { config: config });

			Ok(())
		}
	}

	/// Type to represent a specify the MessageSender implementation for axelar
	/// evm
	pub struct AxelarEvmKind;

	impl<T: Config> MessageSender<AxelarEvmKind> for Pallet<T> {
		type Destination = EVMChainId;
		type Origin = [u8; 20];

		fn send(
			_: AxelarEvmKind,
			origin_address: Self::Origin,
			chain_id: Self::Destination,
			message: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let config = Configuration::<T>::get(chain_id).ok_or(Error::<T>::RouterNotFound)?;

			let sender_evm_address = H160::from_slice(&origin_address);

			let message = wrap_into_axelar_msg(
				message,
				config.target_evm_chain.into_inner(),
				config.liquidity_pools_contract_address,
			)
			.map_err(DispatchError::Other)?;

			T::Transactor::call(
				sender_evm_address,
				config.evm.target_contract_address,
				message.as_slice(),
				config.evm.fee_values.value,
				config.evm.fee_values.gas_price,
				config.evm.fee_values.gas_limit,
			)
		}
	}
}
