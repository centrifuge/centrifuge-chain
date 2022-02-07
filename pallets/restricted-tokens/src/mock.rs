// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

pub use crate as pallet_restricted_tokens;
use common_traits::{PreConditions, TokenMetadata};
use frame_support::parameter_types;
use frame_support::sp_io::TestExternalities;
use frame_support::traits::{Everything, GenesisBuild};
use orml_traits::parameter_type_with_key;
use pallet_restricted_tokens::TransferDetails;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::testing::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

pub const DISTR_PER_ACCOUNT: u64 = 1000;
type AccountId = u64;
type Balance = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;

#[derive(
	codec::Encode,
	codec::Decode,
	Clone,
	Copy,
	Debug,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	scale_info::TypeInfo,
	codec::MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Cfg,
	KUSD,
	USDT,
	RestrictedCoin,
}

impl TokenMetadata for CurrencyId {
	fn name(&self) -> Vec<u8> {
		match self {
			CurrencyId::Cfg => b"Centrifuge".to_vec(),
			CurrencyId::USDT => b"USD Tither".to_vec(),
			CurrencyId::KUSD => b"Acala USD".to_vec(),
			CurrencyId::RestrictedCoin => b"Restricted Token".to_vec(),
		}
	}

	fn symbol(&self) -> Vec<u8> {
		match self {
			CurrencyId::Cfg => b"CFG".to_vec(),
			CurrencyId::USDT => b"USDT".to_vec(),
			CurrencyId::KUSD => b"KUSD".to_vec(),
			CurrencyId::RestrictedCoin => b"RST".to_vec(),
		}
	}

	fn decimals(&self) -> u8 {
		match self {
			CurrencyId::Cfg => 18,
			CurrencyId::USDT => 12,
			CurrencyId::KUSD => 12,
			CurrencyId::RestrictedCoin => 27,
		}
	}
}

// Build mock runtime
frame_support::construct_runtime!(
	pub enum MockRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		OrmlTokens: orml_tokens::{Pallet, Config<T>, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Config<T>, Storage, Event<T>},
		Tokens: pallet_restricted_tokens::{Pallet, Call, Event<T>},
	}
);

// Parameterize frame system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(1024);
}

// Implement frame system configuration for the mock runtime
impl frame_system::Config for MockRuntime {
	type BaseCallFilter = Everything;
	type BlockWeights = BlockWeights;
	type BlockLength = ();
	type Origin = Origin;
	type Index = u64;
	type Call = Call;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		// every currency has a zero existential deposit
		match currency_id {
			_ => 1,
		}
	};
}

parameter_types! {
	pub const MaxLocks: u32 = 100;
	pub const ExistentialDeposit: u64 = 1;
}
impl pallet_balances::Config for MockRuntime {
	type MaxLocks = MaxLocks;
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

impl orml_tokens::Config for MockRuntime {
	type Event = Event;
	type Balance = Balance;
	type Amount = i64;
	type CurrencyId = CurrencyId;
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
}

parameter_types! {
	pub const NativeToken: CurrencyId = CurrencyId::Cfg;
}
impl pallet_restricted_tokens::Config for MockRuntime {
	type Event = Event;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type PreExtrTransfer = RestrictedTokens;
	type PreFungiblesMutate = common_traits::Always;
	type PreFungiblesMutateHold = common_traits::Always;
	type PreFungiblesTransfer = common_traits::Always;
	type Fungibles = OrmlTokens;
	type PreCurrency = common_traits::Always;
	type PreReservableCurrency = common_traits::Always;
	type PreFungibleMutate = common_traits::Always;
	type PreFungibleMutateHold = common_traits::Always;
	type PreFungibleTransfer = common_traits::Always;
	type NativeFungible = Balances;
	type NativeToken = NativeToken;
}

// Restricted coins are only allowed to be send to users with an id over 100
pub struct RestrictedTokens;
impl PreConditions<TransferDetails<AccountId, CurrencyId, Balance>> for RestrictedTokens {
	fn check(t: TransferDetails<AccountId, CurrencyId, Balance>) -> bool {
		match t.id {
			CurrencyId::KUSD | CurrencyId::USDT => true,
			CurrencyId::RestrictedCoin => t.recv >= 100 && t.send >= 100,
			CurrencyId::Cfg => true,
		}
	}
}

pub struct TestExternalitiesBuilder;
// Implement default trait for test externalities builder
impl Default for TestExternalitiesBuilder {
	fn default() -> Self {
		Self {}
	}
}

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub fn build(self, optional: Option<impl FnOnce()>) -> TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<MockRuntime>()
			.unwrap();

		let kusd = (0..10)
			.into_iter()
			.map(|idx| (idx, CurrencyId::KUSD, DISTR_PER_ACCOUNT))
			.collect::<Vec<(AccountId, CurrencyId, Balance)>>();
		let usdt = (0..10)
			.into_iter()
			.map(|idx| (idx, CurrencyId::USDT, DISTR_PER_ACCOUNT))
			.collect::<Vec<(AccountId, CurrencyId, Balance)>>();
		let restric_1 = (0..10)
			.into_iter()
			.map(|idx| (idx, CurrencyId::RestrictedCoin, DISTR_PER_ACCOUNT))
			.collect::<Vec<(AccountId, CurrencyId, Balance)>>();
		let restric_2 = (100..200)
			.into_iter()
			.map(|idx| (idx, CurrencyId::RestrictedCoin, DISTR_PER_ACCOUNT))
			.collect::<Vec<(AccountId, CurrencyId, Balance)>>();

		let mut balances = vec![];
		balances.extend(kusd);
		balances.extend(usdt);
		balances.extend(restric_1);
		balances.extend(restric_2);

		orml_tokens::GenesisConfig::<MockRuntime> { balances }
			.assimilate_storage(&mut storage)
			.unwrap();

		let mut ext = TestExternalities::from(storage);

		if let Some(execute) = optional {
			ext.execute_with(execute);
		}
		ext
	}
}
