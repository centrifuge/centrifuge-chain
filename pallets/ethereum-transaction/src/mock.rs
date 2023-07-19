use std::str::FromStr;

use fp_evm::{FeeCalculator, Precompile, PrecompileResult};
use frame_support::{parameter_types, traits::FindAuthor, weights::Weight};
use pallet_ethereum::IntermediateStateRoot;
use pallet_evm::{
	runner::stack::Runner, AddressMapping, EnsureAddressNever, EnsureAddressRoot,
	FixedGasWeightMapping, PrecompileHandle, PrecompileSet, SubstrateBlockHashMapping,
};
use sp_core::{crypto::AccountId32, ByteArray, ConstU16, ConstU32, ConstU64, H160, H256, U256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	ConsensusEngineId,
};

use crate::pallet as pallet_ethereum_transaction;

pub type Balance = u128;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances,
		EVM: pallet_evm,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage},
		Ethereum: pallet_ethereum::{Pallet, Call, Storage, Event, Origin},
		EthereumTransaction: pallet_ethereum_transaction,
	}
);

frame_support::parameter_types! {
	pub const MaxConnectorsPerDomain: u32 = 3;
}

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = AccountId32;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = ConstU64<250>;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
	type DbWeight = ();
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = ConstU16<42>;
	type SystemWeightInfo = ();
	type Version = ();
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

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
		(1_000_000_000u128.into(), Weight::from_ref_time(7u64))
	}
}

/// Identity address mapping.
pub struct IdentityAddressMapping;

impl AddressMapping<AccountId32> for IdentityAddressMapping {
	fn into_account_id(address: H160) -> AccountId32 {
		let tag = b"EVM";
		let mut bytes = [0; 32];
		bytes[0..20].copy_from_slice(address.as_bytes());
		bytes[20..28].copy_from_slice(&2000u64.to_be_bytes());
		bytes[28..31].copy_from_slice(tag);

		AccountId32::from_slice(bytes.as_slice()).unwrap()
	}
}

const AUTHOR: &'static str = "1234500000000000000000000000000000000000";

pub struct FindAuthorTruncated;
impl FindAuthor<H160> for FindAuthorTruncated {
	fn find_author<'a, I>(_digests: I) -> Option<H160>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		Some(H160::from_str(AUTHOR).unwrap())
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
	fn is_precompile(&self, address: H160) -> bool {
		address == H160::from_low_u64_be(1)
	}
}

parameter_types! {
	pub BlockGasLimit: U256 = U256::max_value();
	pub WeightPerGas: Weight = Weight::from_ref_time(20_000);
	pub MockPrecompiles: MockPrecompileSet = MockPrecompileSet;
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
	type GasWeightMapping = FixedGasWeightMapping<Self>;
	type OnChargeTransaction = ();
	type OnCreate = ();
	type PrecompilesType = MockPrecompileSet;
	type PrecompilesValue = MockPrecompiles;
	type Runner = Runner<Self>;
	type RuntimeEvent = RuntimeEvent;
	type WeightPerGas = WeightPerGas;
	type WithdrawOrigin = EnsureAddressNever<Self::AccountId>;
}

impl pallet_ethereum::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type StateRoot = IntermediateStateRoot<Self>;
}

impl pallet_ethereum_transaction::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| frame_system::Pallet::<Runtime>::set_block_number(1));

	ext
}
