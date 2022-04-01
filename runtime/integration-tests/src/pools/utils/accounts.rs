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

pub use sp_core::sr25519;
use sp_core::{
	sr25519::{Pair, Public, Signature},
	ByteArray, Pair as PairT, H256,
};
use sp_runtime::AccountId32;

/// Set of test accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumIter)]
pub enum Keyring {
	Admin,
	TrancheInvestor(u32),
	Alice,
	Bob,
	Charlie,
	Dave,
	Eve,
	Ferdie,
	Costume(&'static str),
}

impl Keyring {
	pub fn to_h256_public(self) -> H256 {
		Public::from(self).as_array_ref().into()
	}

	pub fn to_raw_public_vec(self) -> Vec<u8> {
		Public::from(self).to_raw_vec()
	}

	pub fn to_account_id(self) -> AccountId32 {
		self.public().0.into()
	}

	pub fn sign(self, msg: &[u8]) -> Signature {
		Pair::from(self).sign(msg)
	}

	pub fn pair(self) -> Pair {
		let path = match self {
			Keyring::Admin => "Admin".to_owned(),
			Keyring::TrancheInvestor(tranche_index) => format!("Tranche{}", tranche_index),
			Keyring::Alice => "Alice".to_owned(),
			Keyring::Bob => "Bob".to_owned(),
			Keyring::Charlie => "Charlie".to_owned(),
			Keyring::Dave => "Dave".to_owned(),
			Keyring::Eve => "Eve".to_owned(),
			Keyring::Ferdie => "Ferdie".to_owned(),
			Keyring::Costume(derivation_path) => derivation_path.to_owned(),
		};

		Pair::from_string(&format!("//{}", path.as_str()), None)
			.expect("static values are known good; qed")
	}

	/// Returns an iterator over all test accounts.
	pub fn iter() -> impl Iterator<Item = Keyring> {
		<Self as strum::IntoEnumIterator>::iter()
	}

	pub fn public(self) -> Public {
		self.pair().public()
	}

	pub fn to_seed(self) -> String {
		let path = match self {
			Keyring::Admin => "Admin".to_owned(),
			Keyring::TrancheInvestor(tranche_index) => format!("Tranche{}", tranche_index),
			Keyring::Alice => "Alice".to_owned(),
			Keyring::Bob => "Bob".to_owned(),
			Keyring::Charlie => "Charlie".to_owned(),
			Keyring::Dave => "Dave".to_owned(),
			Keyring::Eve => "Eve".to_owned(),
			Keyring::Ferdie => "Ferdie".to_owned(),
			Keyring::Costume(derivation_path) => derivation_path.to_owned(),
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

impl From<Keyring> for H256 {
	fn from(k: Keyring) -> Self {
		k.pair().public().0.into()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::{sr25519::Pair, Pair as PairT};

	#[test]
	fn should_work() {
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
