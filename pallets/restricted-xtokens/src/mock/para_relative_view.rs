use cumulus_primitives_core::{ChannelStatus, GetChannelInfo, ParaId};
use frame_support::{
	construct_runtime, match_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
	weights::constants::WEIGHT_PER_SECOND,
};
use frame_system::EnsureRoot;
use orml_traits::{
	location::{AbsoluteReserveProvider, RelativeReserveProvider},
	parameter_type_with_key,
};
use orml_xcm_support::{IsNativeConcrete, MultiCurrencyAdapter, MultiNativeAsset};
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::Sibling;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{Convert, IdentityLookup},
	AccountId32,
};
use xcm::latest::{prelude::*, Weight};
use xcm_builder::{
	AccountId32Aliases, AllowTopLevelPaidExecutionFrom, EnsureXcmOrigin, FixedWeightBounds,
	LocationInverter, ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative,
	SiblingParachainConvertsVia, SignedAccountId32AsNative, SignedToAccountId32,
	SovereignSignedViaLocation, TakeWeightCredit,
};
use xcm_executor::{Config, XcmExecutor};

use super::{Amount, Balance, CurrencyId, CurrencyIdConvert, ParachainXcmRouter};
use crate as restricted_xtokens;
use crate::mock::AllTokensAreCreatedEqualToWeight;

pub type AccountId = AccountId32;

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = AccountId;
	type BaseCallFilter = Everything;
	type BlockHashCount = ConstU64<250>;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
	type Call = Call;
	type DbWeight = ();
	type Event = Event;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type Origin = Origin;
	type PalletInfo = PalletInfo;
	type SS58Prefix = ();
	type SystemWeightInfo = ();
	type Version = ();
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<1>;
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Runtime {
	type Amount = Amount;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = Everything;
	type Event = Event;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<50>;
	type OnDust = ();
	type OnKilledTokenAccount = ();
	type OnNewTokenAccount = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = WEIGHT_PER_SECOND.ref_time() / 4;
	pub const ReservedDmpWeight: Weight = WEIGHT_PER_SECOND.ref_time() / 4;
}

impl parachain_info::Config for Runtime {}

parameter_types! {
	pub const RelayLocation: MultiLocation = MultiLocation::parent();
	pub const RelayNetwork: NetworkId = NetworkId::Kusama;
	pub RelayChainOrigin: Origin = cumulus_pallet_xcm::Origin::Relay.into();
	pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
}

pub type LocationToAccountId = (
	ParentIsPreset<AccountId>,
	SiblingParachainConvertsVia<Sibling, AccountId>,
	AccountId32Aliases<RelayNetwork, AccountId>,
);

pub type XcmOriginToCallOrigin = (
	SovereignSignedViaLocation<LocationToAccountId, Origin>,
	RelayChainAsNative<RelayChainOrigin, Origin>,
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, Origin>,
	SignedAccountId32AsNative<RelayNetwork, Origin>,
	XcmPassthrough<Origin>,
);

pub type LocalAssetTransactor = MultiCurrencyAdapter<
	Tokens,
	(),
	IsNativeConcrete<CurrencyId, CurrencyIdConvert>,
	AccountId,
	LocationToAccountId,
	CurrencyId,
	CurrencyIdConvert,
	(),
>;

pub type XcmRouter = ParachainXcmRouter<ParachainInfo>;
pub type Barrier = (TakeWeightCredit, AllowTopLevelPaidExecutionFrom<Everything>);

pub struct XcmConfig;
impl Config for XcmConfig {
	type AssetClaims = PolkadotXcm;
	type AssetTransactor = LocalAssetTransactor;
	type AssetTrap = PolkadotXcm;
	type Barrier = Barrier;
	type Call = Call;
	type IsReserve = MultiNativeAsset<AbsoluteReserveProvider>;
	type IsTeleporter = ();
	type LocationInverter = LocationInverter<Ancestry>;
	type OriginConverter = XcmOriginToCallOrigin;
	type ResponseHandler = ();
	type SubscriptionService = PolkadotXcm;
	type Trader = AllTokensAreCreatedEqualToWeight;
	type Weigher = FixedWeightBounds<ConstU64<10>, Call, ConstU32<100>>;
	type XcmSender = XcmRouter;
}

pub struct ChannelInfo;
impl GetChannelInfo for ChannelInfo {
	fn get_channel_status(_id: ParaId) -> ChannelStatus {
		ChannelStatus::Ready(10, 10)
	}

	fn get_channel_max(_id: ParaId) -> Option<usize> {
		Some(usize::max_value())
	}
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type ChannelInfo = ChannelInfo;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type ControllerOriginConverter = XcmOriginToCallOrigin;
	type Event = Event;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type VersionWrapper = ();
	type WeightInfo = ();
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type Event = Event;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl cumulus_pallet_xcm::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;

impl pallet_xcm::Config for Runtime {
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type Call = Call;
	type Event = Event;
	type ExecuteXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type LocationInverter = LocationInverter<Ancestry>;
	type Origin = Origin;
	type SendXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type Weigher = FixedWeightBounds<ConstU64<10>, Call, ConstU32<100>>;
	type XcmExecuteFilter = Everything;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type XcmReserveTransferFilter = Everything;
	type XcmRouter = XcmRouter;
	type XcmTeleportFilter = Nothing;

	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
}

pub struct AccountIdToMultiLocation;
impl Convert<AccountId, MultiLocation> for AccountIdToMultiLocation {
	fn convert(account: AccountId) -> MultiLocation {
		X1(Junction::AccountId32 {
			network: NetworkId::Any,
			id: account.into(),
		})
		.into()
	}
}

pub struct RelativeCurrencyIdConvert;
impl Convert<CurrencyId, Option<MultiLocation>> for RelativeCurrencyIdConvert {
	fn convert(id: CurrencyId) -> Option<MultiLocation> {
		match id {
			CurrencyId::R => Some(Parent.into()),
			CurrencyId::A => Some(
				(
					Parent,
					Parachain(1),
					GeneralKey(b"A".to_vec().try_into().unwrap()),
				)
					.into(),
			),
			CurrencyId::A1 => Some(
				(
					Parent,
					Parachain(1),
					GeneralKey(b"A1".to_vec().try_into().unwrap()),
				)
					.into(),
			),
			CurrencyId::B => Some(
				(
					Parent,
					Parachain(2),
					GeneralKey(b"B".to_vec().try_into().unwrap()),
				)
					.into(),
			),
			CurrencyId::B1 => Some(
				(
					Parent,
					Parachain(2),
					GeneralKey(b"B1".to_vec().try_into().unwrap()),
				)
					.into(),
			),
			CurrencyId::B2 => Some(
				(
					Parent,
					Parachain(2),
					GeneralKey(b"B2".to_vec().try_into().unwrap()),
				)
					.into(),
			),
			CurrencyId::C => Some(
				(
					Parent,
					Parachain(3),
					GeneralKey(b"C".to_vec().try_into().unwrap()),
				)
					.into(),
			),
			CurrencyId::D => Some(GeneralKey(b"D".to_vec().try_into().unwrap()).into()),
		}
	}
}
impl Convert<MultiLocation, Option<CurrencyId>> for RelativeCurrencyIdConvert {
	fn convert(l: MultiLocation) -> Option<CurrencyId> {
		let a: Vec<u8> = "A".into();
		let a1: Vec<u8> = "A1".into();
		let b: Vec<u8> = "B".into();
		let b1: Vec<u8> = "B1".into();
		let b2: Vec<u8> = "B2".into();
		let c: Vec<u8> = "C".into();
		let d: Vec<u8> = "D".into();

		let self_para_id: u32 = ParachainInfo::parachain_id().into();
		if l == MultiLocation::parent() {
			return Some(CurrencyId::R);
		}
		match l {
			MultiLocation { parents, interior } if parents == 1 => match interior {
				X2(Parachain(1), GeneralKey(k)) if k == a => Some(CurrencyId::A),
				X2(Parachain(1), GeneralKey(k)) if k == a1 => Some(CurrencyId::A1),
				X2(Parachain(2), GeneralKey(k)) if k == b => Some(CurrencyId::B),
				X2(Parachain(2), GeneralKey(k)) if k == b1 => Some(CurrencyId::B1),
				X2(Parachain(2), GeneralKey(k)) if k == b2 => Some(CurrencyId::B2),
				X2(Parachain(3), GeneralKey(k)) if k == c => Some(CurrencyId::C),
				X2(Parachain(para_id), GeneralKey(k)) if k == d && para_id == self_para_id => {
					Some(CurrencyId::D)
				}
				_ => None,
			},
			MultiLocation { parents, interior } if parents == 0 => match interior {
				X1(GeneralKey(k)) if k == a => Some(CurrencyId::A),
				X1(GeneralKey(k)) if k == b => Some(CurrencyId::B),
				X1(GeneralKey(k)) if k == a1 => Some(CurrencyId::A1),
				X1(GeneralKey(k)) if k == b1 => Some(CurrencyId::B1),
				X1(GeneralKey(k)) if k == b2 => Some(CurrencyId::B2),
				X1(GeneralKey(k)) if k == c => Some(CurrencyId::C),
				X1(GeneralKey(k)) if k == d => Some(CurrencyId::D),
				_ => None,
			},
			_ => None,
		}
	}
}
impl Convert<MultiAsset, Option<CurrencyId>> for RelativeCurrencyIdConvert {
	fn convert(a: MultiAsset) -> Option<CurrencyId> {
		if let MultiAsset {
			fun: Fungible(_),
			id: Concrete(id),
		} = a
		{
			Self::convert(id)
		} else {
			Option::None
		}
	}
}

parameter_types! {
	pub SelfLocation: MultiLocation = MultiLocation::here();
	pub const MaxAssetsForTransfer: usize = 2;
}

match_types! {
	pub type ParentOrParachains: impl Contains<MultiLocation> = {
		MultiLocation { parents: 0, interior: X1(Junction::AccountId32 { .. }) } |
		MultiLocation { parents: 1, interior: X1(Junction::AccountId32 { .. }) } |
		MultiLocation { parents: 1, interior: X2(Parachain(1), Junction::AccountId32 { .. }) } |
		MultiLocation { parents: 1, interior: X2(Parachain(2), Junction::AccountId32 { .. }) } |
		MultiLocation { parents: 1, interior: X2(Parachain(3), Junction::AccountId32 { .. }) } |
		MultiLocation { parents: 1, interior: X2(Parachain(4), Junction::AccountId32 { .. }) } |
		MultiLocation { parents: 1, interior: X2(Parachain(100), Junction::AccountId32 { .. }) }
	};
}

parameter_type_with_key! {
	pub ParachainMinFee: |location: MultiLocation| -> Option<u128> {
		#[allow(clippy::match_ref_pats)] // false positive
		match (location.parents, location.first_interior()) {
			(1, Some(Parachain(2))) => Some(40),
			(1, Some(Parachain(3))) => Some(40),
			_ => None,
		}
	};
}

pub struct Restrictions;
impl cfg_traits::PreConditions<crate::Effects<AccountId, CurrencyId, Balance>> for Restrictions {
	type Result = DispatchResult;

	fn check(effect: Effects<AccountId, CurrencyId, Balance>) -> Self::Result {
		Ok(())
	}
}

impl restricted_xtokens::Config for Runtime {
	type PreExtrTransfer = Restrictions;
}

impl orml_xtokens::Config for Runtime {
	type AccountIdToMultiLocation = AccountIdToMultiLocation;
	type Balance = Balance;
	type BaseXcmWeight = ConstU64<100_000_000>;
	type CurrencyId = CurrencyId;
	type CurrencyIdConvert = RelativeCurrencyIdConvert;
	type Event = Event;
	type LocationInverter = LocationInverter<Ancestry>;
	type MaxAssetsForTransfer = MaxAssetsForTransfer;
	type MinXcmFee = ParachainMinFee;
	type MultiLocationsFilter = ParentOrParachains;
	type ReserveProvider = RelativeReserveProvider;
	type SelfLocation = SelfLocation;
	type Weigher = FixedWeightBounds<ConstU64<10>, Call, ConstU32<100>>;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl orml_xcm::Config for Runtime {
	type Event = Event;
	type SovereignOrigin = EnsureRoot<AccountId>;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},

		ParachainInfo: parachain_info::{Pallet, Storage, Config},
		XcmpQueue: cumulus_pallet_xcmp_queue::{Pallet, Call, Storage, Event<T>},
		DmpQueue: cumulus_pallet_dmp_queue::{Pallet, Call, Storage, Event<T>},
		CumulusXcm: cumulus_pallet_xcm::{Pallet, Event<T>, Origin},

		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		OrmlXTokens: orml_xtokens::{Pallet, Storage, Event<T>},
		XTokens: restricted_xtokens::{Pallet, Call},

		PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
		OrmlXcm: orml_xcm::{Pallet, Call, Event<T>},
	}
);
