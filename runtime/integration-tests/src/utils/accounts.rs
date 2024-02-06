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

// NOTE: Taken mostly from paritytech-substrate

use std::collections::HashMap;

use cfg_primitives::Index;
use ethabi::ethereum_types::{H160, H256};
use frame_support::traits::OriginTrait;
use fudge::primitives::Chain;
use node_primitives::{AccountId as RelayAccountId, Index as RelayIndex};
use sp_core::{ecdsa, ed25519, sr25519, Hasher, Pair as PairT};
use sp_runtime::{AccountId32, MultiSignature};

use crate::{
	chain::{centrifuge, centrifuge::PARA_ID, relay},
	utils::env::TestEnv,
};

/// Struct that takes care of handling nonces for accounts
pub struct NonceManager {
	nonces: HashMap<Chain, HashMap<Keyring, Index>>,
}

impl NonceManager {
	pub fn new() -> Self {
		Self {
			nonces: HashMap::new(),
		}
	}

	/// Retrieves the latest nonce of an account.
	/// If the nonce is not already in the map for a given chain-account
	/// combination it ensures to fetch the latest nonce and store it in the
	/// map.
	///
	/// MUST be executed in an externalites provided env.
	pub fn nonce(&mut self, chain: Chain, who: Keyring) -> Index {
		self.nonces
			.entry(chain)
			.or_insert(HashMap::new())
			.entry(who)
			.or_insert(Self::nonce_from_chain(chain, who))
			.clone()
	}

	fn nonce_from_chain(chain: Chain, who: Keyring) -> Index {
		match chain {
			Chain::Relay => nonce::<relay::Runtime, RelayAccountId, RelayIndex>(
				who.clone().id().into(),
			),
			Chain::Para(id) => match id {
				_ if id == PARA_ID => nonce::<centrifuge::Runtime, cfg_primitives::AccountId, cfg_primitives::Index>(
					who.clone().id().into()
				),
				_ => unreachable!("Currently no nonces for chains differing from Relay and centrifuge are supported. Para ID {}", id)
			}
		}
	}

	/// Retrieves the latest nonce of an account. Returns latest and increases
	/// the nonce by 1.
	/// If the nonce is not already in the map for a given chain-account
	/// combination it ensures to fetch the latest nonce and store it in the
	/// map.
	///
	/// MUST be executed in an externalites provided env.
	pub fn fetch_add(&mut self, chain: Chain, who: Keyring) -> Index {
		let curr = self
			.nonces
			.entry(chain)
			.or_insert(HashMap::new())
			.entry(who)
			.or_insert(Self::nonce_from_chain(chain, who));
		let next = curr.clone();
		*curr = *curr + 1;
		next
	}

	/// Increases the nonce by one. If it is not existing fails.
	pub fn incr(&mut self, chain: Chain, who: Keyring) -> Result<(), ()> {
		let curr = self
			.nonces
			.get_mut(&chain)
			.ok_or(())?
			.get_mut(&who)
			.ok_or(())?;
		*curr = *curr + 1;
		Ok(())
	}
}

/// Retrieves a nonce from the centrifuge state
///
/// **NOTE: Usually one should use the TestEnv::nonce() api**
fn nonce_centrifuge(env: &TestEnv, who: Keyring) -> cfg_primitives::Index {
	env.centrifuge
		.with_state(|| {
			nonce::<centrifuge::Runtime, cfg_primitives::AccountId, cfg_primitives::Index>(
				who.clone().id().into(),
			)
		})
		.expect("ESSENTIAL: Nonce must be retrievable.")
}

/// Retrieves a nonce from the relay state
///
/// **NOTE: Usually one should use the TestEnv::nonce() api**
fn nonce_relay(env: &TestEnv, who: Keyring) -> RelayIndex {
	env.relay
		.with_state(|| nonce::<relay::Runtime, RelayAccountId, RelayIndex>(who.clone().id().into()))
		.expect("ESSENTIAL: Nonce must be retrievable.")
}

fn nonce<Runtime, AccountId, Index>(who: AccountId) -> Index
where
	Runtime: frame_system::Config,
	AccountId: Into<<Runtime as frame_system::Config>::AccountId>,
	Index: From<<Runtime as frame_system::Config>::Index>,
{
	frame_system::Pallet::<Runtime>::account_nonce(who.into()).into()
}

/// Set of test accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyring {
	Admin,
	TrancheInvestor(u32),
	Alice,
	Bob,
	Charlie,
	Dave,
	Eve,
	Ferdie,
	Custom(&'static str),
}

impl Keyring {
	pub fn id(self) -> AccountId32 {
		let pair: sr25519::Pair = self.into();
		pair.public().into()
	}

	pub fn id_ed25519(self) -> AccountId32 {
		let pair: ed25519::Pair = self.into();
		pair.public().into()
	}

	pub fn id_ecdsa<T: pallet_evm_chain_id::Config>(self) -> AccountId32 {
		let h160: H160 = self.into();

		runtime_common::account_conversion::AccountConverter::<(), ()>::convert_evm_address(
			pallet_evm_chain_id::ChainId::<T>::get(),
			h160.0,
		)
	}

	pub fn as_multi(self) -> sp_runtime::MultiSigner {
		let pair: sr25519::Pair = self.into();
		pair.public().into()
	}

	pub fn as_multi_ed25519(self) -> sp_runtime::MultiSigner {
		let pair: ed25519::Pair = self.into();
		pair.public().into()
	}

	pub fn as_multi_ecdsa(self) -> sp_runtime::MultiSigner {
		let pair: ecdsa::Pair = self.into();
		pair.public().into()
	}

	pub fn sign(self, msg: &[u8]) -> sr25519::Signature {
		let pair: sr25519::Pair = self.into();
		pair.sign(msg)
	}

	pub fn sign_ed25519(self, msg: &[u8]) -> ed25519::Signature {
		let pair: ed25519::Pair = self.into();
		pair.sign(msg)
	}

	pub fn sign_ecdsa(self, msg: &[u8]) -> ecdsa::Signature {
		let pair: ecdsa::Pair = self.into();
		pair.sign(msg)
	}

	pub fn as_origin<T: OriginTrait<AccountId = AccountId32>>(self) -> T {
		OriginTrait::signed(self.id())
	}

	pub fn as_origin_ed25519<T: OriginTrait<AccountId = AccountId32>>(self) -> T {
		OriginTrait::signed(self.id_ed25519())
	}

	pub fn as_origin_ecdsa<
		R: pallet_evm_chain_id::Config,
		T: OriginTrait<AccountId = AccountId32>,
	>(
		self,
	) -> T {
		OriginTrait::signed(self.id_ecdsa::<R>())
	}

	pub fn to_seed(self) -> String {
		let path = match self {
			Keyring::Admin => "Admin".to_owned(),
			Keyring::TrancheInvestor(tranche_index) => format!("Tranche{tranche_index}"),
			Keyring::Alice => "Alice".to_owned(),
			Keyring::Bob => "Bob".to_owned(),
			Keyring::Charlie => "Charlie".to_owned(),
			Keyring::Dave => "Dave".to_owned(),
			Keyring::Eve => "Eve".to_owned(),
			Keyring::Ferdie => "Ferdie".to_owned(),
			Keyring::Custom(derivation_path) => derivation_path.to_owned(),
		};
		format!("//{}", path.as_str())
	}
}

impl From<Keyring> for AccountId32 {
	fn from(value: Keyring) -> Self {
		value.id()
	}
}

impl From<Keyring> for [u8; 32] {
	fn from(value: Keyring) -> Self {
		value.id().into()
	}
}

impl From<Keyring> for H160 {
	fn from(value: Keyring) -> Self {
		H160::from(H256::from(
			sp_core::KeccakHasher::hash(&Into::<ecdsa::Pair>::into(value).public().as_ref()).0,
		))
	}
}

impl From<Keyring> for sp_core::H160 {
	fn from(value: Keyring) -> Self {
		sp_core::H160::from(sp_core::H256::from(sp_core::KeccakHasher::hash(
			&Into::<ecdsa::Pair>::into(value).public().as_ref(),
		)))
	}
}

impl From<Keyring> for sp_runtime::MultiAddress<AccountId32, ()> {
	fn from(x: Keyring) -> Self {
		sp_runtime::MultiAddress::Id(x.into())
	}
}

impl From<Keyring> for sr25519::Public {
	fn from(k: Keyring) -> Self {
		Into::<sr25519::Pair>::into(k).public()
	}
}

impl From<Keyring> for sr25519::Pair {
	fn from(k: Keyring) -> Self {
		sr25519::Pair::from_string(&k.to_seed(), None).expect("static values are known good; qed")
	}
}

impl From<Keyring> for ed25519::Public {
	fn from(k: Keyring) -> Self {
		Into::<ed25519::Pair>::into(k).public()
	}
}

impl From<Keyring> for ed25519::Pair {
	fn from(k: Keyring) -> Self {
		ed25519::Pair::from_string(&k.to_seed(), None).expect("static values are known good; qed")
	}
}

impl From<Keyring> for ecdsa::Public {
	fn from(k: Keyring) -> Self {
		Into::<ecdsa::Pair>::into(k).public()
	}
}

impl From<Keyring> for ecdsa::Pair {
	fn from(k: Keyring) -> Self {
		ecdsa::Pair::from_string(&k.to_seed(), None).expect("static values are known good; qed")
	}
}

#[derive(Debug)]
pub struct ParseKeyringError;
impl std::fmt::Display for ParseKeyringError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "ParseKeyringError")
	}
}

impl std::str::FromStr for Keyring {
	type Err = ParseKeyringError;

	fn from_str(s: &str) -> Result<Self, <Self as std::str::FromStr>::Err> {
		match s {
			"alice" => Ok(Keyring::Alice),
			"bob" => Ok(Keyring::Bob),
			"charlie" => Ok(Keyring::Charlie),
			"dave" => Ok(Keyring::Dave),
			"eve" => Ok(Keyring::Eve),
			"ferdie" => Ok(Keyring::Ferdie),
			"admin" => Ok(Keyring::Admin),
			_ => Err(ParseKeyringError),
		}
	}
}

/// Returns a Vector of default accounts
///
/// Accounts:
/// * Keyring::Admin
/// * Keyring::Alice
/// * Keyring::Bob
/// * Keyring::Ferdie
/// * Keyring::Charlie
/// * Keyring::Dave
/// * Keyring::Eve
/// * Keyring::TrancheInvestor(1)
/// * Keyring::TrancheInvestor(2)
/// * Keyring::TrancheInvestor(3)
/// * Keyring::TrancheInvestor(4)
/// * Keyring::TrancheInvestor(5)
/// * Keyring::TrancheInvestor(6)
/// * Keyring::TrancheInvestor(7)
/// * Keyring::TrancheInvestor(8)
/// * Keyring::TrancheInvestor(9)
/// * Keyring::TrancheInvestor(10)
/// * Keyring::TrancheInvestor(11)
/// * Keyring::TrancheInvestor(12)
/// * Keyring::TrancheInvestor(13)
/// * Keyring::TrancheInvestor(14)
/// * Keyring::TrancheInvestor(15)
/// * Keyring::TrancheInvestor(16)
/// * Keyring::TrancheInvestor(17)
/// * Keyring::TrancheInvestor(18)
/// * Keyring::TrancheInvestor(19)
/// * Keyring::TrancheInvestor(20)
/// * Keyring::TrancheInvestor(21)
/// * Keyring::TrancheInvestor(22)
/// * Keyring::TrancheInvestor(23)
/// * Keyring::TrancheInvestor(24)
/// * Keyring::TrancheInvestor(25)
/// * Keyring::TrancheInvestor(26)
/// * Keyring::TrancheInvestor(27)
/// * Keyring::TrancheInvestor(28)
/// * Keyring::TrancheInvestor(29)
/// * Keyring::TrancheInvestor(30)
/// * Keyring::TrancheInvestor(31)
/// * Keyring::TrancheInvestor(32)
/// * Keyring::TrancheInvestor(33)
/// * Keyring::TrancheInvestor(34)
/// * Keyring::TrancheInvestor(35)
/// * Keyring::TrancheInvestor(36)
/// * Keyring::TrancheInvestor(37)
/// * Keyring::TrancheInvestor(38)
/// * Keyring::TrancheInvestor(39)
/// * Keyring::TrancheInvestor(40)
/// * Keyring::TrancheInvestor(41)
/// * Keyring::TrancheInvestor(42)
/// * Keyring::TrancheInvestor(43)
/// * Keyring::TrancheInvestor(44)
/// * Keyring::TrancheInvestor(45)
/// * Keyring::TrancheInvestor(46)
/// * Keyring::TrancheInvestor(47)
/// * Keyring::TrancheInvestor(48)
/// * Keyring::TrancheInvestor(49)
/// * Keyring::TrancheInvestor(50)
pub fn default_accounts() -> Vec<Keyring> {
	let mut standard = vec![
		Keyring::Admin,
		Keyring::Alice,
		Keyring::Bob,
		Keyring::Ferdie,
		Keyring::Charlie,
		Keyring::Dave,
		Keyring::Eve,
	];
	standard.extend(default_investors());
	standard
}

/// Returns a Vector of default investor accounts
///
/// Accounts:
/// * Keyring::TrancheInvestor(1)
/// * Keyring::TrancheInvestor(2)
/// * Keyring::TrancheInvestor(3)
/// * Keyring::TrancheInvestor(4)
/// * Keyring::TrancheInvestor(5)
/// * Keyring::TrancheInvestor(6)
/// * Keyring::TrancheInvestor(7)
/// * Keyring::TrancheInvestor(8)
/// * Keyring::TrancheInvestor(9)
/// * Keyring::TrancheInvestor(10)
/// * Keyring::TrancheInvestor(11)
/// * Keyring::TrancheInvestor(12)
/// * Keyring::TrancheInvestor(13)
/// * Keyring::TrancheInvestor(14)
/// * Keyring::TrancheInvestor(15)
/// * Keyring::TrancheInvestor(16)
/// * Keyring::TrancheInvestor(17)
/// * Keyring::TrancheInvestor(18)
/// * Keyring::TrancheInvestor(19)
/// * Keyring::TrancheInvestor(20)
/// * Keyring::TrancheInvestor(21)
/// * Keyring::TrancheInvestor(22)
/// * Keyring::TrancheInvestor(23)
/// * Keyring::TrancheInvestor(24)
/// * Keyring::TrancheInvestor(25)
/// * Keyring::TrancheInvestor(26)
/// * Keyring::TrancheInvestor(27)
/// * Keyring::TrancheInvestor(28)
/// * Keyring::TrancheInvestor(29)
/// * Keyring::TrancheInvestor(30)
/// * Keyring::TrancheInvestor(31)
/// * Keyring::TrancheInvestor(32)
/// * Keyring::TrancheInvestor(33)
/// * Keyring::TrancheInvestor(34)
/// * Keyring::TrancheInvestor(35)
/// * Keyring::TrancheInvestor(36)
/// * Keyring::TrancheInvestor(37)
/// * Keyring::TrancheInvestor(38)
/// * Keyring::TrancheInvestor(39)
/// * Keyring::TrancheInvestor(40)
/// * Keyring::TrancheInvestor(41)
/// * Keyring::TrancheInvestor(42)
/// * Keyring::TrancheInvestor(43)
/// * Keyring::TrancheInvestor(44)
/// * Keyring::TrancheInvestor(45)
/// * Keyring::TrancheInvestor(46)
/// * Keyring::TrancheInvestor(47)
/// * Keyring::TrancheInvestor(48)
/// * Keyring::TrancheInvestor(49)
/// * Keyring::TrancheInvestor(50)
pub fn default_investors() -> Vec<Keyring> {
	vec![
		Keyring::TrancheInvestor(1),
		Keyring::TrancheInvestor(2),
		Keyring::TrancheInvestor(3),
		Keyring::TrancheInvestor(4),
		Keyring::TrancheInvestor(5),
		Keyring::TrancheInvestor(6),
		Keyring::TrancheInvestor(7),
		Keyring::TrancheInvestor(8),
		Keyring::TrancheInvestor(9),
		Keyring::TrancheInvestor(10),
		Keyring::TrancheInvestor(11),
		Keyring::TrancheInvestor(12),
		Keyring::TrancheInvestor(13),
		Keyring::TrancheInvestor(14),
		Keyring::TrancheInvestor(15),
		Keyring::TrancheInvestor(16),
		Keyring::TrancheInvestor(17),
		Keyring::TrancheInvestor(18),
		Keyring::TrancheInvestor(19),
		Keyring::TrancheInvestor(20),
		Keyring::TrancheInvestor(21),
		Keyring::TrancheInvestor(22),
		Keyring::TrancheInvestor(23),
		Keyring::TrancheInvestor(24),
		Keyring::TrancheInvestor(25),
		Keyring::TrancheInvestor(26),
		Keyring::TrancheInvestor(27),
		Keyring::TrancheInvestor(28),
		Keyring::TrancheInvestor(29),
		Keyring::TrancheInvestor(30),
		Keyring::TrancheInvestor(31),
		Keyring::TrancheInvestor(32),
		Keyring::TrancheInvestor(33),
		Keyring::TrancheInvestor(34),
		Keyring::TrancheInvestor(35),
		Keyring::TrancheInvestor(36),
		Keyring::TrancheInvestor(37),
		Keyring::TrancheInvestor(38),
		Keyring::TrancheInvestor(39),
		Keyring::TrancheInvestor(40),
		Keyring::TrancheInvestor(41),
		Keyring::TrancheInvestor(42),
		Keyring::TrancheInvestor(43),
		Keyring::TrancheInvestor(44),
		Keyring::TrancheInvestor(45),
		Keyring::TrancheInvestor(46),
		Keyring::TrancheInvestor(47),
		Keyring::TrancheInvestor(48),
		Keyring::TrancheInvestor(49),
		Keyring::TrancheInvestor(50),
	]
}

#[cfg(test)]
mod tests {
	use sp_core::{sr25519::Pair, Pair as PairT};

	use super::*;

	#[test]
	fn keyring_works() {
		assert!(Pair::verify(
			&Keyring::Alice.sign(b"I am Alice!"),
			b"I am Alice!",
			&Keyring::Alice.into(),
		));
		assert!(!Pair::verify(
			&Keyring::Alice.sign(b"I am Alice!"),
			b"I am Bob!",
			&Keyring::Alice.into(),
		));
		assert!(!Pair::verify(
			&Keyring::Alice.sign(b"I am Alice!"),
			b"I am Alice!",
			&Keyring::Bob.into(),
		));
	}
}
