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

use ethabi::ethereum_types::{H160, H256};
use frame_support::traits::OriginTrait;
use sp_core::{ecdsa, ed25519, sr25519, Hasher, Pair as PairT};
use sp_runtime::AccountId32;

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

	/// NOTE: Needs to be executed in an externalities environment
	pub fn id_ecdsa<T: pallet_evm_chain_id::Config>(self) -> AccountId32 {
		runtime_common::account_conversion::AccountConverter::evm_to_account::<T>(self.into())
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

impl From<Keyring> for [u8; 20] {
	fn from(value: Keyring) -> Self {
		sp_core::H160::from(sp_core::H256::from(sp_core::KeccakHasher::hash(
			&Into::<ecdsa::Pair>::into(value).public().as_ref(),
		)))
		.0
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
pub fn default_investors() -> Vec<Keyring> {
	(0..=50).map(Keyring::TrancheInvestor).collect()
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
