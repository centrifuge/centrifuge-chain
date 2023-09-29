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

use std::{collections::HashMap, marker::PhantomData};

use cfg_primitives::{AccountId, Index};
use frame_support::Never;
use fudge::primitives::Chain;
use node_primitives::{AccountId as RelayAccountId, Index as RelayIndex};
use sp_core::{
	sr25519::{Pair, Public, Signature},
	Pair as PairT,
};
use sp_runtime::{traits::Hash, AccountId32};

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
				who.clone().to_account_id().into(),
			),
			Chain::Para(id) => match id {
				_ if id == PARA_ID => nonce::<centrifuge::Runtime, cfg_primitives::AccountId, cfg_primitives::Index>(
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
				who.clone().to_account_id().into(),
			)
		})
		.expect("ESSENTIAL: Nonce must be retrievable.")
}

/// Retrieves a nonce from the relay state
///
/// **NOTE: Usually one should use the TestEnv::nonce() api**
fn nonce_relay(env: &TestEnv, who: Keyring) -> RelayIndex {
	env.relay
		.with_state(|| {
			nonce::<relay::Runtime, RelayAccountId, RelayIndex>(who.clone().to_account_id().into())
		})
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
pub enum Keyring<T: Crypto = Sr25519> {
	__Ignore(
		frame_support::sp_std::marker::PhantomData<(T)>,
		crate::utils::Never,
	),
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

pub use sr25519::Sr25519;
pub mod sr25519 {
	use sp_core::{
		crypto::AccountId32,
		sr25519::{Pair, Public, Signature},
		Pair as PairT,
	};

	use crate::utils::accounts::{Crypto, Keyring};

	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
	pub struct Sr25519;

	impl Crypto for Sr25519 {}

	impl Keyring<Sr25519> {
		pub fn to_account_id(self) -> AccountId32 {
			self.public().0.into()
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
				Keyring::__Ignore(..) => unreachable!("Variant can not be instantiated. qed."),
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
				Keyring::__Ignore(..) => unreachable!("Variant can not be instantiated. qed."),
			};
			format!("//{}", path.as_str())
		}

		/// Create a crypto `Pair` from a numeric value.
		pub fn numeric(idx: usize) -> Pair {
			Pair::from_string(&format!("//{}", idx), None)
				.expect("numeric values are known good; qed")
		}

		/// Get account id of a `numeric` account.
		pub fn numeric_id(idx: usize) -> AccountId32 {
			(*Self::numeric(idx).public().as_array_ref()).into()
		}
	}

	impl From<Keyring<Sr25519>> for sp_runtime::MultiSigner {
		fn from(x: Keyring<Sr25519>) -> Self {
			sp_runtime::MultiSigner::Sr25519(x.into())
		}
	}

	impl From<Keyring<Sr25519>> for sp_runtime::MultiAddress<AccountId32, ()> {
		fn from(x: Keyring<Sr25519>) -> Self {
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
	pub fn default_accounts() -> Vec<Keyring<Sr25519>> {
		vec![
			Keyring::Admin,
			Keyring::Alice,
			Keyring::Bob,
			Keyring::Ferdie,
			Keyring::Charlie,
			Keyring::Dave,
			Keyring::Eve,
		]
	}

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
	pub fn full_accounts() -> Vec<Keyring<Sr25519>> {
		let mut accounts = default_accounts();
		accounts.extend(default_investors());
		accounts
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
	pub fn default_investors() -> Vec<Keyring<Sr25519>> {
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

	impl From<Keyring<Sr25519>> for AccountId32 {
		fn from(k: Keyring<Sr25519>) -> Self {
			k.to_account_id()
		}
	}

	impl From<Keyring<Sr25519>> for Public {
		fn from(k: Keyring<Sr25519>) -> Self {
			k.pair().public()
		}
	}

	impl From<Keyring<Sr25519>> for Pair {
		fn from(k: Keyring<Sr25519>) -> Self {
			k.pair()
		}
	}

	impl From<Keyring<Sr25519>> for [u8; 32] {
		fn from(k: Keyring<Sr25519>) -> Self {
			k.pair().public().0
		}
	}

	impl From<Keyring<Sr25519>> for crate::chain::centrifuge::RuntimeOrigin {
		fn from(account: Keyring<Sr25519>) -> Self {
			crate::chain::centrifuge::RuntimeOrigin::signed(AccountId32::from(account))
		}
	}

	impl From<Keyring<Sr25519>> for crate::chain::relay::RuntimeOrigin {
		fn from(account: Keyring<Sr25519>) -> Self {
			crate::chain::relay::RuntimeOrigin::signed(AccountId32::from(account))
		}
	}
}

pub use ecdsa::Ecdsa;
mod ecdsa {
	use sp_core::{
		crypto::AccountId32,
		ecdsa::{Pair, Public, Signature},
		Hasher, Pair as PairT, H160, H256,
	};

	use super::{Crypto, Keyring};
	use crate::chain::centrifuge;

	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
	pub struct Ecdsa;

	impl Crypto for Ecdsa {}

	impl Keyring<Ecdsa> {
		pub fn to_account_id(self) -> AccountId32 {
			let h160 = H160::from(H256::from(sp_core::KeccakHasher::hash(
				&self.public().as_ref(),
			)));

			runtime_common::account_conversion::AccountConverter::<(), ()>::convert_evm_address(
				centrifuge::CHAIN_ID,
				h160.0,
			)
		}

		pub fn to_h160(self) -> H160 {
			H160::from(H256::from(sp_core::KeccakHasher::hash(
				&self.public().as_ref(),
			)))
		}

		pub fn sign(self, msg: &[u8]) -> Signature {
			sp_core::Pair::sign(&Pair::from(self.pair()), msg)
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
				Keyring::__Ignore(..) => unreachable!("Variant can not be instantiated. qed."),
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
				Keyring::__Ignore(..) => unreachable!("Variant can not be instantiated. qed."),
			};
			format!("//{}", path.as_str())
		}

		/// Create a crypto `Pair` from a numeric value.
		pub fn numeric(idx: usize) -> Pair {
			Pair::from_string(&format!("//{}", idx), None)
				.expect("numeric values are known good; qed")
		}

		/// Get account id of a `numeric` account.
		pub fn numeric_id(idx: usize) -> AccountId32 {
			let id = Self::numeric(idx);
			let h160 = H160::from(H256::from(sp_core::KeccakHasher::hash(
				&id.public().as_ref(),
			)));

			runtime_common::account_conversion::AccountConverter::<(), ()>::convert_evm_address(
				centrifuge::CHAIN_ID,
				h160.0,
			)
		}
	}

	impl From<Keyring<Ecdsa>> for sp_runtime::MultiSigner {
		fn from(x: Keyring<Ecdsa>) -> Self {
			sp_runtime::MultiSigner::Ecdsa(x.pair().into())
		}
	}

	impl From<Keyring<Ecdsa>> for sp_runtime::MultiAddress<AccountId32, ()> {
		fn from(x: Keyring<Ecdsa>) -> Self {
			sp_runtime::MultiAddress::Id(x.to_account_id())
		}
	}

	#[derive(Debug)]
	pub struct ParseKeyringError;
	impl std::fmt::Display for ParseKeyringError {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			write!(f, "ParseKeyringError")
		}
	}

	impl std::str::FromStr for Keyring<Ecdsa> {
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
	pub fn default_accounts() -> Vec<Keyring<Ecdsa>> {
		vec![
			Keyring::Admin,
			Keyring::Alice,
			Keyring::Bob,
			Keyring::Ferdie,
			Keyring::Charlie,
			Keyring::Dave,
			Keyring::Eve,
		]
	}

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
	pub fn full_accounts() -> Vec<Keyring<Ecdsa>> {
		let mut accounts = default_accounts();
		accounts.extend(default_investors());
		accounts
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
	pub fn default_investors() -> Vec<Keyring<Ecdsa>> {
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

	impl From<Keyring<Ecdsa>> for AccountId32 {
		fn from(k: Keyring<Ecdsa>) -> Self {
			k.to_account_id()
		}
	}

	impl From<Keyring<Ecdsa>> for Public {
		fn from(k: Keyring<Ecdsa>) -> Self {
			k.pair().public()
		}
	}

	impl From<Keyring<Ecdsa>> for Pair {
		fn from(k: Keyring<Ecdsa>) -> Self {
			k.pair()
		}
	}

	impl From<Keyring<Ecdsa>> for crate::chain::centrifuge::RuntimeOrigin {
		fn from(account: Keyring<Ecdsa>) -> Self {
			crate::chain::centrifuge::RuntimeOrigin::signed(AccountId32::from(account))
		}
	}

	impl From<Keyring<Ecdsa>> for crate::chain::relay::RuntimeOrigin {
		fn from(account: Keyring<Ecdsa>) -> Self {
			crate::chain::relay::RuntimeOrigin::signed(AccountId32::from(account))
		}
	}
}

pub use ed25519::Ed25519;
mod ed25519 {
	use sp_core::{
		crypto::AccountId32,
		ed25519::{Pair, Public, Signature},
		Pair as PairT,
	};

	use super::{Crypto, Keyring};

	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
	pub struct Ed25519;

	impl Crypto for Ed25519 {}

	impl Keyring<Ed25519> {
		pub fn to_account_id(self) -> AccountId32 {
			self.public().0.into()
		}

		pub fn sign(self, msg: &[u8]) -> Signature {
			sp_core::Pair::sign(&Pair::from(self.pair()), msg)
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
				Keyring::__Ignore(..) => unreachable!("Variant can not be instantiated. qed."),
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
				Keyring::__Ignore(..) => unreachable!("Variant can not be instantiated. qed."),
			};
			format!("//{}", path.as_str())
		}

		/// Create a crypto `Pair` from a numeric value.
		pub fn numeric(idx: usize) -> Pair {
			Pair::from_string(&format!("//{}", idx), None)
				.expect("numeric values are known good; qed")
		}

		/// Get account id of a `numeric` account.
		pub fn numeric_id(idx: usize) -> AccountId32 {
			(*Self::numeric(idx).public().as_array_ref()).into()
		}
	}

	impl From<Keyring<Ed25519>> for sp_runtime::MultiSigner {
		fn from(x: Keyring<Ed25519>) -> Self {
			sp_runtime::MultiSigner::Ed25519(x.pair().into())
		}
	}

	impl From<Keyring<Ed25519>> for sp_runtime::MultiAddress<AccountId32, ()> {
		fn from(x: Keyring<Ed25519>) -> Self {
			sp_runtime::MultiAddress::Id(x.to_account_id())
		}
	}

	#[derive(Debug)]
	pub struct ParseKeyringError;
	impl std::fmt::Display for ParseKeyringError {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			write!(f, "ParseKeyringError")
		}
	}

	impl std::str::FromStr for Keyring<Ed25519> {
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
	pub fn default_accounts() -> Vec<Keyring<Ed25519>> {
		vec![
			Keyring::Admin,
			Keyring::Alice,
			Keyring::Bob,
			Keyring::Ferdie,
			Keyring::Charlie,
			Keyring::Dave,
			Keyring::Eve,
		]
	}

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
	pub fn full_accounts() -> Vec<Keyring<Ed25519>> {
		let mut accounts = default_accounts();
		accounts.extend(default_investors());
		accounts
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
	pub fn default_investors() -> Vec<Keyring<Ed25519>> {
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

	impl From<Keyring<Ed25519>> for AccountId32 {
		fn from(k: Keyring<Ed25519>) -> Self {
			k.to_account_id()
		}
	}

	impl From<Keyring<Ed25519>> for Public {
		fn from(k: Keyring<Ed25519>) -> Self {
			k.pair().public()
		}
	}

	impl From<Keyring<Ed25519>> for Pair {
		fn from(k: Keyring<Ed25519>) -> Self {
			k.pair()
		}
	}

	impl From<Keyring<Ed25519>> for [u8; 32] {
		fn from(k: Keyring<Ed25519>) -> Self {
			k.pair().public().0
		}
	}

	impl From<Keyring<Ed25519>> for crate::chain::centrifuge::RuntimeOrigin {
		fn from(account: Keyring<Ed25519>) -> Self {
			crate::chain::centrifuge::RuntimeOrigin::signed(AccountId32::from(account))
		}
	}

	impl From<Keyring<Ed25519>> for crate::chain::relay::RuntimeOrigin {
		fn from(account: Keyring<Ed25519>) -> Self {
			crate::chain::relay::RuntimeOrigin::signed(AccountId32::from(account))
		}
	}
}

pub trait Crypto: sp_std::fmt::Debug + Clone + Copy + PartialEq + Eq {}

pub fn all_accounts() -> Vec<AccountId> {
	let mut accounts = Vec::new();
	accounts.extend(
		sr25519::full_accounts()
			.into_iter()
			.map(|account| account.to_account_id()),
	);
	accounts.extend(
		ecdsa::full_accounts()
			.into_iter()
			.map(|account| account.to_account_id()),
	);
	accounts.extend(
		ed25519::full_accounts()
			.into_iter()
			.map(|account| account.to_account_id()),
	);
	accounts
}

#[cfg(test)]
mod tests {
	use sp_core::Pair as PairT;

	use super::*;

	#[test]
	fn keyring_works() {
		// Sr25519
		assert!(sp_core::sr25519::Pair::verify(
			&Keyring::<Sr25519>::Alice.sign(b"I am Alice!"),
			b"I am Alice!",
			&Keyring::<Sr25519>::Alice.public(),
		));
		assert!(!sp_core::sr25519::Pair::verify(
			&Keyring::<Sr25519>::Alice.sign(b"I am Alice!"),
			b"I am Bob!",
			&Keyring::<Sr25519>::Alice.public(),
		));
		assert!(!sp_core::sr25519::Pair::verify(
			&Keyring::<Sr25519>::Alice.sign(b"I am Alice!"),
			b"I am Alice!",
			&Keyring::<Sr25519>::Bob.public(),
		));

		// Ed25519
		assert!(sp_core::ed25519::Pair::verify(
			&Keyring::<Ed25519>::Alice.sign(b"I am Alice!"),
			b"I am Alice!",
			&Keyring::<Ed25519>::Alice.public(),
		));
		assert!(!sp_core::ed25519::Pair::verify(
			&Keyring::<Ed25519>::Alice.sign(b"I am Alice!"),
			b"I am Bob!",
			&Keyring::<Ed25519>::Alice.public(),
		));
		assert!(!sp_core::ed25519::Pair::verify(
			&Keyring::<Ed25519>::Alice.sign(b"I am Alice!"),
			b"I am Alice!",
			&Keyring::<Ed25519>::Bob.public(),
		));

		// Ecdsa
		assert!(sp_core::ecdsa::Pair::verify(
			&Keyring::<Ecdsa>::Alice.sign(b"I am Alice!"),
			b"I am Alice!",
			&Keyring::<Ecdsa>::Alice.public(),
		));
		assert!(!sp_core::ecdsa::Pair::verify(
			&Keyring::<Ecdsa>::Alice.sign(b"I am Alice!"),
			b"I am Bob!",
			&Keyring::<Ecdsa>::Alice.public(),
		));
		assert!(!sp_core::ecdsa::Pair::verify(
			&Keyring::<Ecdsa>::Alice.sign(b"I am Alice!"),
			b"I am Alice!",
			&Keyring::<Ecdsa>::Bob.public(),
		));
	}
}
