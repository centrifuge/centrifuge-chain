use std::str::FromStr;

use cfg_mocks::{pallet_mock_liquidity_pools, pallet_mock_routers};
use cfg_primitives::{BLOCK_STORAGE_LIMIT, MAX_POV_SIZE};
use cfg_types::domain_address::DomainAddress;
use frame_support::{derive_impl, parameter_types, traits::FindAuthor, weights::Weight};
use pallet_ethereum::{IntermediateStateRoot, PostLogContent};
use pallet_evm::{
	runner::stack::Runner, AddressMapping, EnsureAddressNever, EnsureAddressRoot, FeeCalculator,
	FixedGasWeightMapping, IsPrecompileResult, Precompile, PrecompileHandle, PrecompileResult,
	PrecompileSet, SubstrateBlockHashMapping,
};
use sp_core::{crypto::AccountId32, ByteArray, ConstU32, H160, U256};
use sp_runtime::{traits::IdentityLookup, ConsensusEngineId};

pub type Balance = u128;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Balances: pallet_balances,
		MockLiquidityPools: pallet_mock_liquidity_pools,
		EVM: pallet_evm,
		Timestamp: pallet_timestamp,
		Ethereum: pallet_ethereum,
		EthereumTransaction: pallet_ethereum_transaction,
	}
);

frame_support::parameter_types! {
	pub const MaxIncomingMessageSize: u32 = 1024;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = AccountId32;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type Lookup = IdentityLookup<Self::AccountId>;
}

parameter_types! {
	// the minimum fee for an anchor is 500,000ths of a CFG.
	// This is set to a value so you can still get some return without getting your account removed.
	pub const ExistentialDeposit: Balance = 1 * cfg_primitives::MICRO_CFG;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type RuntimeHoldReason = ();
}

impl pallet_mock_liquidity_pools::Config for Runtime {
	type DomainAddress = DomainAddress;
	type Message = ();
}

impl pallet_ethereum_transaction::Config for Runtime {}

impl pallet_mock_routers::Config for Runtime {}

parameter_types! {
	pub const MinimumPeriod: u64 = 1000;
}

impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = MinimumPeriod;
	type Moment = u64;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

///////////////////////
// EVM pallet mocks. //
///////////////////////

pub struct FixedGasPrice;
impl FeeCalculator for FixedGasPrice {
	fn min_gas_price() -> (U256, Weight) {
		// Return some meaningful gas price and weight
		(1_000_000_000u128.into(), Weight::from_parts(7u64, 0))
	}
}

/// Identity address mapping.
pub struct IdentityAddressMapping;

impl AddressMapping<AccountId32> for IdentityAddressMapping {
	fn into_account_id(address: H160) -> AccountId32 {
		let tag = b"EVM";
		let mut bytes = [0; 32];
		bytes[0..20].copy_from_slice(address.as_bytes());
		bytes[20..28].copy_from_slice(&1u64.to_be_bytes());
		bytes[28..31].copy_from_slice(tag);

		AccountId32::from_slice(bytes.as_slice()).unwrap()
	}
}

pub struct FindAuthorTruncated;
impl FindAuthor<H160> for FindAuthorTruncated {
	fn find_author<'a, I>(_digests: I) -> Option<H160>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		Some(H160::from_str("1234500000000000000000000000000000000000").unwrap())
	}
}

pub struct MockPrecompileSet;

impl PrecompileSet for MockPrecompileSet {
	/// Tries to execute a precompile in the precompile set.
	/// If the provided address is not a precompile, returns None.
	fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
		let address = handle.code_address();

		if address == H160::from_low_u64_be(1) {
			return Some(pallet_evm_precompile_simple::Identity::execute(handle));
		}

		None
	}

	/// Check if the given address is a precompile. Should only be called to
	/// perform the check while not executing the precompile afterward, since
	/// `execute` already performs a check internally.
	fn is_precompile(&self, address: H160, _remaining_gas: u64) -> IsPrecompileResult {
		IsPrecompileResult::Answer {
			is_precompile: address == H160::from_low_u64_be(1),
			extra_cost: 0,
		}
	}
}

parameter_types! {
	pub BlockGasLimit: U256 = U256::max_value();
	pub WeightPerGas: Weight = Weight::from_parts(20_000, 0);
	pub MockPrecompiles: MockPrecompileSet = MockPrecompileSet;
	pub GasLimitPovSizeRatio: u64 = {
		let block_gas_limit = BlockGasLimit::get().min(u64::MAX.into()).low_u64();
		block_gas_limit.saturating_div(MAX_POV_SIZE)
	};
	pub GasLimitStorageGrowthRatio: u64 =
		BlockGasLimit::get().min(u64::MAX.into()).low_u64().saturating_div(BLOCK_STORAGE_LIMIT);
}

impl pallet_evm::Config for Runtime {
	type AddressMapping = IdentityAddressMapping;
	type BlockGasLimit = BlockGasLimit;
	type BlockHashMapping = SubstrateBlockHashMapping<Self>;
	type CallOrigin = EnsureAddressRoot<Self::AccountId>;
	type ChainId = ();
	type Currency = Balances;
	type FeeCalculator = FixedGasPrice;
	type FindAuthor = FindAuthorTruncated;
	type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
	type GasLimitStorageGrowthRatio = GasLimitStorageGrowthRatio;
	type GasWeightMapping = FixedGasWeightMapping<Self>;
	type OnChargeTransaction = ();
	type OnCreate = ();
	type PrecompilesType = MockPrecompileSet;
	type PrecompilesValue = MockPrecompiles;
	type Runner = Runner<Self>;
	type RuntimeEvent = RuntimeEvent;
	type SuicideQuickClearLimit = ConstU32<0>;
	type Timestamp = Timestamp;
	type WeightInfo = ();
	type WeightPerGas = WeightPerGas;
	type WithdrawOrigin = EnsureAddressNever<Self::AccountId>;
}

parameter_types! {
	pub const PostBlockAndTxnHashes: PostLogContent = PostLogContent::BlockAndTxnHashes;
	pub const ExtraDataLength: u32 = 30;
}

impl pallet_ethereum::Config for Runtime {
	type ExtraDataLength = ExtraDataLength;
	type PostLogContent = PostBlockAndTxnHashes;
	type RuntimeEvent = RuntimeEvent;
	type StateRoot = IntermediateStateRoot<Self>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	System::externalities()
}
