use std::str::FromStr;

use cfg_mocks::{pallet_mock_liquidity_pools, pallet_mock_routers, MessageMock, RouterMock};
use cfg_primitives::{OutboundMessageNonce, BLOCK_STORAGE_LIMIT, MAX_POV_SIZE};
use cfg_traits::TryConvert;
use cfg_types::domain_address::DomainAddress;
use cumulus_primitives_core::{
	Instruction, MultiAsset, MultiLocation, PalletInstance, Parachain, SendError, Xcm, XcmHash,
};
use frame_support::{
	parameter_types,
	traits::{FindAuthor, PalletInfo as PalletInfoTrait},
	weights::Weight,
};
use frame_system::EnsureRoot;
use pallet_ethereum::{IntermediateStateRoot, PostLogContent};
use pallet_evm::{
	runner::stack::Runner, AddressMapping, EnsureAddressNever, EnsureAddressRoot, FeeCalculator,
	FixedGasWeightMapping, IsPrecompileResult, Precompile, PrecompileHandle, PrecompileResult,
	PrecompileSet, SubstrateBlockHashMapping,
};
use pallet_liquidity_pools_gateway::EnsureLocal;
use parity_scale_codec::{Decode, Encode};
use sp_core::{crypto::AccountId32, ByteArray, ConstU16, ConstU32, ConstU64, H160, H256, U256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	ConsensusEngineId, DispatchError,
};
use sp_std::{cell::RefCell, marker::PhantomData};
use xcm::{
	latest::{
		Error as XcmError, InteriorMultiLocation, NetworkId, Result as XcmResult, SendResult,
		XcmContext,
	},
	v3::{Junction, Junctions, MultiAssets, SendXcm},
};
use xcm_executor::{
	traits::{TransactAsset, WeightBounds},
	Assets,
};
use xcm_primitives::{
	HrmpAvailableCalls, HrmpEncodeCall, UtilityAvailableCalls, UtilityEncodeCall, XcmTransact,
};

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
		MockLiquidityPools: pallet_mock_liquidity_pools,
		LiquidityPoolsGateway: pallet_liquidity_pools_gateway,
		XcmTransactor: pallet_xcm_transactor::{Pallet, Call, Event<T>},
		EVM: pallet_evm::{Pallet, Call, Storage, Config, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage},
		Ethereum: pallet_ethereum::{Pallet, Call, Storage, Event, Origin},
		EthereumTransaction: pallet_ethereum_transaction,
	}
);

frame_support::parameter_types! {
	pub const MaxInstancesPerDomain: u32 = 3;
	pub const MaxIncomingMessageSize: u32 = 1024;
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

parameter_types! {
	// the minimum fee for an anchor is 500,000ths of a CFG.
	// This is set to a value so you can still get some return without getting your account removed.
	pub const ExistentialDeposit: Balance = 1 * cfg_primitives::MICRO_CFG;
	// For weight estimation, we assume that the most locks on an individual account will be 50.
	pub const MaxHolds: u32 = 50;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type FreezeIdentifier = ();
	type HoldIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = MaxHolds;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl pallet_mock_liquidity_pools::Config for Runtime {
	type DomainAddress = DomainAddress;
	type Message = MessageMock;
}

impl pallet_ethereum_transaction::Config for Runtime {}

impl pallet_mock_routers::Config for Runtime {}

pub struct MockOriginRecovery;
impl TryConvert<(Vec<u8>, Vec<u8>), DomainAddress> for MockOriginRecovery {
	type Error = DispatchError;

	fn try_convert(_: (Vec<u8>, Vec<u8>)) -> Result<DomainAddress, Self::Error> {
		Err(DispatchError::Other("Unimplemented"))
	}
}

parameter_types! {
	pub Sender: AccountId32 = AccountId32::from(H256::from_low_u64_be(1).to_fixed_bytes());
}

impl pallet_liquidity_pools_gateway::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId32>;
	type InboundQueue = MockLiquidityPools;
	type LocalEVMOrigin = EnsureLocal;
	type MaxIncomingMessageSize = MaxIncomingMessageSize;
	type Message = MessageMock;
	type OriginRecovery = MockOriginRecovery;
	type OutboundMessageNonce = OutboundMessageNonce;
	type Router = RouterMock<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Sender = Sender;
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
///////////////////////////
// XCM transactor mocks. //
///////////////////////////

// Transactors for the mock runtime. Only relay chain
#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, scale_info::TypeInfo)]
pub enum Transactors {
	Relay,
}

#[cfg(feature = "runtime-benchmarks")]
impl Default for Transactors {
	fn default() -> Self {
		Transactors::Relay
	}
}

impl XcmTransact for Transactors {
	fn destination(self) -> MultiLocation {
		match self {
			Transactors::Relay => MultiLocation::parent(),
		}
	}
}

impl UtilityEncodeCall for Transactors {
	fn encode_call(self, call: UtilityAvailableCalls) -> Vec<u8> {
		match self {
			Transactors::Relay => match call {
				UtilityAvailableCalls::AsDerivative(a, b) => {
					let mut call =
						RelayCall::Utility(UtilityCall::AsDerivative(a.clone())).encode();
					call.append(&mut b.clone());
					call
				}
			},
		}
	}
}

pub struct AccountIdToMultiLocation;
impl sp_runtime::traits::Convert<AccountId32, MultiLocation> for AccountIdToMultiLocation {
	fn convert(_account: AccountId32) -> MultiLocation {
		let as_h160: H160 = H160::repeat_byte(0xAA);
		MultiLocation::new(
			0,
			Junctions::X1(Junction::AccountKey20 {
				network: None,
				key: as_h160.as_fixed_bytes().clone(),
			}),
		)
	}
}

pub struct DummyAssetTransactor;
impl TransactAsset for DummyAssetTransactor {
	fn deposit_asset(_what: &MultiAsset, _who: &MultiLocation, _context: &XcmContext) -> XcmResult {
		Ok(())
	}

	fn withdraw_asset(
		_what: &MultiAsset,
		_who: &MultiLocation,
		_context: Option<&XcmContext>,
	) -> Result<Assets, XcmError> {
		Ok(Assets::default())
	}
}

pub struct CurrencyIdToMultiLocation;

pub type AssetId = u128;

#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, scale_info::TypeInfo)]
pub enum CurrencyId {
	SelfReserve,
	OtherReserve(AssetId),
}

impl sp_runtime::traits::Convert<CurrencyId, Option<MultiLocation>> for CurrencyIdToMultiLocation {
	fn convert(currency: CurrencyId) -> Option<MultiLocation> {
		match currency {
			CurrencyId::SelfReserve => {
				let multi: MultiLocation = SelfReserve::get();
				Some(multi)
			}
			// To distinguish between relay and others, specially for reserve asset
			CurrencyId::OtherReserve(asset) => {
				if asset == 0 {
					Some(MultiLocation::parent())
				} else {
					Some(MultiLocation::new(1, Junctions::X1(Parachain(2))))
				}
			}
		}
	}
}

pub struct MockHrmpEncoder;

impl HrmpEncodeCall for MockHrmpEncoder {
	fn hrmp_encode_call(call: HrmpAvailableCalls) -> Result<Vec<u8>, XcmError> {
		match call {
			HrmpAvailableCalls::InitOpenChannel(_, _, _) => {
				Ok(RelayCall::Hrmp(HrmpCall::Init()).encode())
			}
			HrmpAvailableCalls::AcceptOpenChannel(_) => {
				Ok(RelayCall::Hrmp(HrmpCall::Accept()).encode())
			}
			HrmpAvailableCalls::CloseChannel(_) => Ok(RelayCall::Hrmp(HrmpCall::Close()).encode()),
			HrmpAvailableCalls::CancelOpenRequest(_, _) => {
				Ok(RelayCall::Hrmp(HrmpCall::Close()).encode())
			}
		}
	}
}

// Simulates sending a XCM message
thread_local! {
	pub static SENT_XCM: RefCell<Vec<(MultiLocation, xcm::v3::opaque::Xcm)>> = RefCell::new(Vec::new());
}
pub fn sent_xcm() -> Vec<(MultiLocation, xcm::v3::opaque::Xcm)> {
	SENT_XCM.with(|q| (*q.borrow()).clone())
}
pub struct TestSendXcm;
impl SendXcm for TestSendXcm {
	type Ticket = ();

	fn validate(
		destination: &mut Option<MultiLocation>,
		message: &mut Option<xcm::v3::opaque::Xcm>,
	) -> SendResult<Self::Ticket> {
		SENT_XCM.with(|q| {
			q.borrow_mut()
				.push((destination.clone().unwrap(), message.clone().unwrap()))
		});
		Ok(((), MultiAssets::new()))
	}

	fn deliver(_: Self::Ticket) -> Result<XcmHash, SendError> {
		Ok(XcmHash::default())
	}
}

#[derive(Encode, Decode)]
pub enum RelayCall {
	#[codec(index = 0u8)]
	// the index should match the position of the module in `construct_runtime!`
	Utility(UtilityCall),
	#[codec(index = 1u8)]
	// the index should match the position of the module in `construct_runtime!`
	Hrmp(HrmpCall),
}

#[derive(Encode, Decode)]
pub enum UtilityCall {
	#[codec(index = 0u8)]
	AsDerivative(u16),
}

#[derive(Encode, Decode)]
pub enum HrmpCall {
	#[codec(index = 0u8)]
	Init(),
	#[codec(index = 1u8)]
	Accept(),
	#[codec(index = 2u8)]
	Close(),
}

pub type MaxHrmpRelayFee = xcm_builder::Case<MaxFee>;

pub struct DummyWeigher<C>(PhantomData<C>);

impl<C: Decode> WeightBounds<C> for DummyWeigher<C> {
	fn weight(_message: &mut Xcm<C>) -> Result<xcm::latest::Weight, ()> {
		Ok(Weight::zero())
	}

	fn instr_weight(_instruction: &Instruction<C>) -> Result<xcm::latest::Weight, ()> {
		Ok(Weight::zero())
	}
}

parameter_types! {
		pub const RelayNetwork: NetworkId = NetworkId::Polkadot;

		pub ParachainId: cumulus_primitives_core::ParaId = 100.into();

		pub SelfLocation: MultiLocation =
			MultiLocation::new(1, Junctions::X1(Parachain(ParachainId::get().into())));

		pub SelfReserve: MultiLocation = MultiLocation::new(
			1,
			Junctions::X2(
				Parachain(ParachainId::get().into()),
				PalletInstance(
					<Runtime as frame_system::Config>::PalletInfo::index::<Balances>().unwrap() as u8
				)
		));

		pub const BaseXcmWeight: xcm::latest::Weight = xcm::latest::Weight::from_parts(1000, 0);

		pub MaxFee: MultiAsset = (MultiLocation::parent(), 1_000_000_000_000u128).into();

		pub UniversalLocation: InteriorMultiLocation = RelayNetwork::get().into();
}

impl pallet_xcm_transactor::Config for Runtime {
	type AccountIdToMultiLocation = AccountIdToMultiLocation;
	type AssetTransactor = DummyAssetTransactor;
	type Balance = Balance;
	type BaseXcmWeight = BaseXcmWeight;
	type CurrencyId = CurrencyId;
	type CurrencyIdToMultiLocation = CurrencyIdToMultiLocation;
	type DerivativeAddressRegistrationOrigin = EnsureRoot<AccountId32>;
	type HrmpEncoder = MockHrmpEncoder;
	type HrmpManipulatorOrigin = EnsureRoot<AccountId32>;
	type MaxHrmpFee = MaxHrmpRelayFee;
	type ReserveProvider = orml_traits::location::RelativeReserveProvider;
	type RuntimeEvent = RuntimeEvent;
	type SelfLocation = SelfLocation;
	type SovereignAccountDispatcherOrigin = EnsureRoot<AccountId32>;
	type Transactor = Transactors;
	type UniversalLocation = UniversalLocation;
	type Weigher = DummyWeigher<RuntimeCall>;
	type WeightInfo = ();
	type XcmSender = TestSendXcm;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| frame_system::Pallet::<Runtime>::set_block_number(1));

	ext
}
