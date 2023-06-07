use std::str::FromStr;

use cfg_mocks::{pallet_mock_connectors, DomainRouterMock, MessageMock};
use codec::{Decode, Encode};
use frame_support::{parameter_types, traits::FindAuthor, weights::Weight};
use frame_system::EnsureRoot;
use pallet_connectors_gateway::EnsureLocal;
use pallet_evm::{
	runner::stack::Runner, AddressMapping, EnsureAddressNever, EnsureAddressRoot, FeeCalculator,
	FixedGasWeightMapping, Precompile, PrecompileHandle, PrecompileResult, PrecompileSet,
	SubstrateBlockHashMapping,
};
use sp_core::{crypto::AccountId32, ByteArray, ConstU16, ConstU32, ConstU64, H160, H256, U256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	ConsensusEngineId,
};
use sp_std::cell::RefCell;
use xcm::{
	latest::{Error as XcmError, Result as XcmResult},
	v1::{Junction, Junctions, MultiAsset, MultiLocation, NetworkId},
};
use xcm_executor::{
	traits::{InvertLocation, TransactAsset, WeightBounds},
	Assets,
};
use xcm_primitives::{
	HrmpAvailableCalls, HrmpEncodeCall, UtilityAvailableCalls, UtilityEncodeCall, XcmTransact,
	XcmV2Weight,
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
		MockConnectors: pallet_mock_connectors,
		ConnectorsGateway: pallet_connectors_gateway,
		XcmTransactor: pallet_xcm_transactor::{Pallet, Call, Event<T>},
		EVM: pallet_evm::{Pallet, Call, Storage, Config, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage},
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

impl pallet_mock_connectors::Config for Runtime {}

impl pallet_connectors_gateway::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId32>;
	type Connectors = MockConnectors;
	type LocalOrigin = EnsureLocal;
	type MaxConnectorsPerDomain = MaxConnectorsPerDomain;
	type Message = MessageMock;
	type Router = DomainRouterMock<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
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
		AccountId32::from_slice(address.as_fixed_bytes()).unwrap()
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
	type PrecompilesType = MockPrecompileSet;
	type PrecompilesValue = MockPrecompiles;
	type Runner = Runner<Self>;
	type RuntimeEvent = RuntimeEvent;
	type WeightPerGas = WeightPerGas;
	type WithdrawOrigin = EnsureAddressNever<Self::AccountId>;
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
				network: NetworkId::Any,
				key: as_h160.as_fixed_bytes().clone(),
			}),
		)
	}
}

pub struct DummyAssetTransactor;
impl TransactAsset for DummyAssetTransactor {
	fn deposit_asset(_what: &MultiAsset, _who: &MultiLocation) -> XcmResult {
		Ok(())
	}

	fn withdraw_asset(_what: &MultiAsset, _who: &MultiLocation) -> Result<Assets, XcmError> {
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
					Some(MultiLocation::new(1, Junctions::X1(Junction::Parachain(2))))
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
		}
	}
}

pub struct InvertNothing;
impl InvertLocation for InvertNothing {
	fn invert_location(_: &MultiLocation) -> sp_std::result::Result<MultiLocation, ()> {
		Ok(MultiLocation::here())
	}

	fn ancestry() -> MultiLocation {
		MultiLocation::here()
	}
}

// Simulates sending a XCM message
thread_local! {
	pub static SENT_XCM: RefCell<Vec<(MultiLocation, opaque::Xcm)>> = RefCell::new(Vec::new());
}
pub fn sent_xcm() -> Vec<(MultiLocation, opaque::Xcm)> {
	SENT_XCM.with(|q| (*q.borrow()).clone())
}
pub struct TestSendXcm;
impl SendXcm for TestSendXcm {
	fn send_xcm(dest: impl Into<MultiLocation>, msg: opaque::Xcm) -> SendResult {
		SENT_XCM.with(|q| q.borrow_mut().push((dest.into(), msg)));
		Ok(())
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

use sp_std::marker::PhantomData;
use xcm::v2::{opaque, Instruction, SendResult, SendXcm, Xcm};

pub struct DummyWeigher<C>(PhantomData<C>);

impl<C: Decode> WeightBounds<C> for DummyWeigher<C> {
	fn weight(_message: &mut Xcm<C>) -> Result<XcmV2Weight, ()> {
		Ok(0)
	}

	fn instr_weight(_instruction: &Instruction<C>) -> Result<XcmV2Weight, ()> {
		Ok(0)
	}
}

parameter_types! {
		pub ParachainId: cumulus_primitives_core::ParaId = 100.into();

		pub SelfLocation: MultiLocation = (1, Junctions::X1(Junction::Parachain(ParachainId::get().into()))).into();
		pub SelfReserve: MultiLocation = (
		1,
		Junctions::X2(
			Junction::Parachain(ParachainId::get().into()),
			Junction::PalletInstance(0)
		)).into();

		pub const BaseXcmWeight: XcmV2Weight = 1000;

		pub MaxFee: MultiAsset = (MultiLocation::parent(), 1_000_000_000_000u128).into();
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
	type LocationInverter = InvertNothing;
	type MaxHrmpFee = MaxHrmpRelayFee;
	type ReserveProvider = orml_traits::location::RelativeReserveProvider;
	type RuntimeEvent = RuntimeEvent;
	type SelfLocation = SelfLocation;
	type SovereignAccountDispatcherOrigin = EnsureRoot<AccountId32>;
	type Transactor = Transactors;
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
