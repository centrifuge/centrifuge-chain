// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

#![cfg(test)]

use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_io::TestExternalities;
use sp_runtime::{traits::Zero, AccountId32, BoundedVec};
use xcm_executor::{traits::WeightTrader, Assets};
use xcm_simulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain, TestExt};

use super::*;
use crate as restricted_xtokens;

pub mod para;
pub mod relay;

pub const RESTRICTED_SENDER: AccountId32 = AccountId32::new([0u8; 32]);
pub const BOB: AccountId32 = AccountId32::new([1u8; 32]);
pub const RESTRICTED_RECEIVER: AccountId32 = AccountId32::new([2u8; 32]);

fn para_a_rreceiver_relay() -> MultiLocation {
	MultiLocation::new(
		1,
		X1(Junction::AccountId32 {
			network: None,
			id: RESTRICTED_RECEIVER.into(),
		}),
	)
}

fn para_a_rreceiver_para_a() -> MultiLocation {
	MultiLocation::new(
		0,
		X1(Junction::AccountId32 {
			network: None,
			id: RESTRICTED_RECEIVER.into(),
		}),
	)
}

fn para_a_rreceiver_para_b() -> MultiLocation {
	MultiLocation::new(
		1,
		X2(
			Junction::Parachain(2),
			Junction::AccountId32 {
				network: None,
				id: RESTRICTED_RECEIVER.into(),
			},
		),
	)
}

#[derive(
	Encode,
	Decode,
	Eq,
	PartialEq,
	Copy,
	Clone,
	RuntimeDebug,
	PartialOrd,
	Ord,
	codec::MaxEncodedLen,
	TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	/// Relay chain token.
	R,
	/// Parachain A token.
	A,
	/// Parachain A A1 token.
	A1,
	/// Parachain B token.
	B,
	/// Parachain B B1 token
	B1,
}

pub struct CurrencyIdConvert;
impl sp_runtime::traits::Convert<CurrencyId, Option<MultiLocation>> for CurrencyIdConvert {
	fn convert(id: CurrencyId) -> Option<MultiLocation> {
		match id {
			CurrencyId::R => Some(Parent.into()),
			CurrencyId::A => Some(
				(
					Parent,
					Parachain(1),
					Junction::from(BoundedVec::try_from(b"A".to_vec()).unwrap()),
				)
					.into(),
			),
			CurrencyId::A1 => Some(
				(
					Parent,
					Parachain(1),
					Junction::from(BoundedVec::try_from(b"A1".to_vec()).unwrap()),
				)
					.into(),
			),
			CurrencyId::B => Some(
				(
					Parent,
					Parachain(2),
					Junction::from(BoundedVec::try_from(b"B".to_vec()).unwrap()),
				)
					.into(),
			),
			CurrencyId::B1 => Some(
				(
					Parent,
					Parachain(2),
					Junction::from(BoundedVec::try_from(b"B1".to_vec()).unwrap()),
				)
					.into(),
			),
		}
	}
}
impl sp_runtime::traits::Convert<MultiLocation, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(l: MultiLocation) -> Option<CurrencyId> {
		let mut a: Vec<u8> = "A".into();
		a.resize(32, 0);
		let mut a1: Vec<u8> = "A1".into();
		a1.resize(32, 0);
		let mut b: Vec<u8> = "B".into();
		b.resize(32, 0);
		let mut b1: Vec<u8> = "B1".into();
		b1.resize(32, 0);
		if l == MultiLocation::parent() {
			return Some(CurrencyId::R);
		}
		match l {
			MultiLocation { parents, interior } if parents == 1 => match interior {
				X2(Parachain(1), GeneralKey { data, .. }) if data.to_vec() == a => {
					Some(CurrencyId::A)
				}
				X2(Parachain(1), GeneralKey { data, .. }) if data.to_vec() == a1 => {
					Some(CurrencyId::A1)
				}
				X2(Parachain(2), GeneralKey { data, .. }) if data.to_vec() == b => {
					Some(CurrencyId::B)
				}
				X2(Parachain(2), GeneralKey { data, .. }) if data.to_vec() == b1 => {
					Some(CurrencyId::B1)
				}
				_ => None,
			},
			MultiLocation { parents, interior } if parents == 0 => match interior {
				X1(GeneralKey { data, .. }) if data.to_vec() == a => Some(CurrencyId::A),
				X1(GeneralKey { data, .. }) if data.to_vec() == b => Some(CurrencyId::B),
				X1(GeneralKey { data, .. }) if data.to_vec() == a1 => Some(CurrencyId::A1),
				X1(GeneralKey { data, .. }) if data.to_vec() == b1 => Some(CurrencyId::B1),
				_ => None,
			},
			_ => None,
		}
	}
}
impl sp_runtime::traits::Convert<MultiAsset, Option<CurrencyId>> for CurrencyIdConvert {
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

pub type Balance = u128;
pub type Amount = i128;

decl_test_parachain! {
	pub struct ParaA {
		Runtime = para::Runtime,
		XcmpMessageHandler = para::XcmpQueue,
		DmpMessageHandler = para::DmpQueue,
		new_ext = para_ext(1),
	}
}

decl_test_parachain! {
	pub struct ParaB {
		Runtime = para::Runtime,
		XcmpMessageHandler = para::XcmpQueue,
		DmpMessageHandler = para::DmpQueue,
		new_ext = para_ext(2),
	}
}

decl_test_relay_chain! {
	pub struct Relay {
		Runtime = relay::Runtime,
		RuntimeCall = relay::RuntimeCall,
		RuntimeEvent = relay::RuntimeEvent,
		XcmConfig = relay::XcmConfig,
		MessageQueue = relay::MessageQueue,
		System = relay::System,
		new_ext = relay_ext(),
	}
}

decl_test_network! {
	pub struct TestNet {
		relay_chain = Relay,
		parachains = vec![
			(1, ParaA),
			(2, ParaB),
		],
	}
}

pub type RelayBalances = pallet_balances::Pallet<relay::Runtime>;

pub type ParaTokens = orml_tokens::Pallet<para::Runtime>;
pub type ParaXTokens = restricted_xtokens::Pallet<para::Runtime>;

pub fn para_ext(para_id: u32) -> TestExternalities {
	use para::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let parachain_info_config = parachain_info::GenesisConfig {
		parachain_id: para_id.into(),
	};
	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(
		&parachain_info_config,
		&mut t,
	)
	.unwrap();

	orml_tokens::GenesisConfig::<Runtime> {
		balances: vec![
			(RESTRICTED_SENDER, CurrencyId::R, 1_000),
			(RESTRICTED_SENDER, CurrencyId::A, 1_000),
			(RESTRICTED_SENDER, CurrencyId::A1, 1_000),
			(RESTRICTED_SENDER, CurrencyId::B, 1_000),
			(RESTRICTED_SENDER, CurrencyId::B1, 1_000),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn relay_ext() -> sp_io::TestExternalities {
	use relay::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(RESTRICTED_SENDER, 1_000)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

/// A trader who believes all tokens are created equal to "weight" of any chain,
/// which is not true, but good enough to mock the fee payment of XCM execution.
///
/// This mock will always trade `n` amount of weight to `n` amount of tokens.
pub struct AllTokensAreCreatedEqualToWeight(MultiLocation);
impl WeightTrader for AllTokensAreCreatedEqualToWeight {
	fn new() -> Self {
		Self(MultiLocation::parent())
	}

	fn buy_weight(&mut self, weight: Weight, payment: Assets) -> Result<Assets, XcmError> {
		let asset_id = payment
			.fungible
			.iter()
			.next()
			.expect("Payment must be something; qed")
			.0;
		let required = MultiAsset {
			id: asset_id.clone(),
			fun: Fungible(weight.ref_time() as u128),
		};

		if let MultiAsset {
			fun: _,
			id: Concrete(ref id),
		} = &required
		{
			self.0 = id.clone();
		}

		let unused = payment
			.checked_sub(required)
			.map_err(|_| XcmError::TooExpensive)?;
		Ok(unused)
	}

	fn refund_weight(&mut self, weight: Weight) -> Option<MultiAsset> {
		if weight.is_zero() {
			None
		} else {
			Some((self.0.clone(), weight.ref_time() as u128).into())
		}
	}
}