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

use cfg_primitives::Nonce;
use fudge::primitives::Chain;
use polkadot_core_primitives::{AccountId as RelayAccountId, Nonce as RelayNonce};
pub use sp_core::sr25519;
use sp_core::{
	sr25519::{Pair, Public, Signature},
	Pair as PairT,
};
use sp_runtime::AccountId32;

/*
use crate::{
	chain::{centrifuge, centrifuge::PARA_ID, relay},
	utils::env::TestEnv,
};

/// Struct that takes care of handling nonces for accounts
pub struct NonceManager {
	nonces: HashMap<Chain, HashMap<Keyring, Nonce>>,
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
	pub fn nonce(&mut self, chain: Chain, who: Keyring) -> Nonce {
		self.nonces
			.entry(chain)
			.or_insert(HashMap::new())
			.entry(who)
			.or_insert(Self::nonce_from_chain(chain, who))
			.clone()
	}

	fn nonce_from_chain(chain: Chain, who: Keyring) -> Nonce {
		match chain {
			Chain::Relay => nonce::<relay::Runtime, RelayAccountId, RelayNonce>(
				who.clone().to_account_id().into(),
			),
			Chain::Para(id) => match id {
				_ if id == PARA_ID => nonce::<centrifuge::Runtime, cfg_primitives::AccountId, Nonce>(
					who.clone().to_account_id().into()
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
	pub fn fetch_add(&mut self, chain: Chain, who: Keyring) -> Nonce {
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
fn nonce_centrifuge(env: &TestEnv, who: Keyring) -> Nonce {
	env.centrifuge
		.with_state(|| {
			nonce::<centrifuge::Runtime, cfg_primitives::AccountId, Nonce>(
				who.clone().to_account_id().into(),
			)
		})
		.expect("ESSENTIAL: Nonce must be retrievable.")
}

/// Retrieves a nonce from the relay state
///
/// **NOTE: Usually one should use the TestEnv::nonce() api**
fn nonce_relay(env: &TestEnv, who: Keyring) -> RelayNonce {
	env.relay
		.with_state(|| {
			nonce::<relay::Runtime, RelayAccountId, RelayNonce>(who.clone().to_account_id().into())
		})
		.expect("ESSENTIAL: Nonce must be retrievable.")
}
*/

fn nonce<Runtime, AccountId, Nonce>(who: AccountId) -> Nonce
where
	Runtime: frame_system::Config,
	AccountId: Into<<Runtime as frame_system::Config>::AccountId>,
	Nonce: From<<Runtime as frame_system::Config>::Nonce>,
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
	pub fn to_account_id(self) -> AccountId32 {
		self.public().0.into()
	}

	/// Shorter alias for `to_account_id()`
	pub fn id(self) -> AccountId32 {
		self.to_account_id()
	}

	pub fn sign(self, msg: &[u8]) -> Signature {
		Pair::from(self).sign(msg)
	}

	pub fn pair(self) -> Pair {
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

		Pair::from_string(&format!("//{}", path.as_str()), None)
			.expect("static values are known good; qed")
	}

	pub fn public(self) -> Public {
		self.pair().public()
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

	/// Create a crypto `Pair` from a numeric value.
	pub fn numeric(idx: usize) -> Pair {
		Pair::from_string(&format!("//{}", idx), None).expect("numeric values are known good; qed")
	}

	/// Get account id of a `numeric` account.
	pub fn numeric_id(idx: usize) -> AccountId32 {
		(*Self::numeric(idx).public().as_array_ref()).into()
	}
}

impl From<Keyring> for sp_runtime::MultiSigner {
	fn from(x: Keyring) -> Self {
		sp_runtime::MultiSigner::Sr25519(x.into())
	}
}

impl From<Keyring> for sp_runtime::MultiAddress<AccountId32, ()> {
	fn from(x: Keyring) -> Self {
		sp_runtime::MultiAddress::Id(x.into())
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

impl From<Keyring> for AccountId32 {
	fn from(k: Keyring) -> Self {
		k.to_account_id()
	}
}

impl From<Keyring> for Public {
	fn from(k: Keyring) -> Self {
		k.pair().public()
	}
}

impl From<Keyring> for Pair {
	fn from(k: Keyring) -> Self {
		k.pair()
	}
}

impl From<Keyring> for [u8; 32] {
	fn from(k: Keyring) -> Self {
		k.pair().public().0
	}
}

impl From<Keyring> for crate::chain::centrifuge::RuntimeOrigin {
	fn from(account: Keyring) -> Self {
		crate::chain::centrifuge::RuntimeOrigin::signed(AccountId32::from(account))
	}
}

impl From<Keyring> for crate::chain::relay::RuntimeOrigin {
	fn from(account: Keyring) -> Self {
		crate::chain::relay::RuntimeOrigin::signed(AccountId32::from(account))
	}
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
			&Keyring::Alice.public(),
		));
		assert!(!Pair::verify(
			&Keyring::Alice.sign(b"I am Alice!"),
			b"I am Bob!",
			&Keyring::Alice.public(),
		));
		assert!(!Pair::verify(
			&Keyring::Alice.sign(b"I am Alice!"),
			b"I am Alice!",
			&Keyring::Bob.public(),
		));
	}
}
