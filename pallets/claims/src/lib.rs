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

//! # Claims processing pallet
//!
//! This pallet implement a reward claim mechanism with vesting, for
//! rewarding tokens (CFG or others) awarded through [Tinlake](https://tinlake.centrifuge.io)
//! investments.
//!
//! ## Overview
//! This pallet is used for processing reward payout claims from investors who
//! invested in [Tinlake](https://tinlake.centrifuge.io) pools.
//!
//! ## Terminology
//! CFG is the native token of Centrifuge chain.
//!
//! ## Usage
//!
//! ## Interface
//!
//! ### Supported Origins
//! Valid origin is an administrator or root.
//!
//! ### Types
//! - `AdminOrigin` - Ensure that origin of a transaction is an administrator.
//! - `Currency` -  Expected currency of the reward claim.
//! - `Event` -  Overarching type for pallet events.
//! - `MinimalPayoutAmount` -  Minimal reward payout amount that can be claimed.
//! - `Longevity` -  Expected longevity of a validated unsigned extrinsic.
//! - `PalletId` -  Constant configuration parameter to store the module identifier for the pallet.
//! - `UnsignedPriority` -  A configuration for base priority of unsigned transactions.
//! - `WeightInfo` -  Pallet's transaction weights, calculated using runtime benchmarking.
//!
//! ### Events
//! - `Claimed(T::AccountId, <T as pallet_balances::Config>::Balance)` - Event triggered after a reward claim is successfully processed.
//! - `RootHashStored(<T as frame_system::Config>::Hash)` - Event triggered when the root hash is stored.
//!
//! ### Errors
//! - `InsufficientBalance` - Amount being claimed is less than the available amount in [`ClaimedAmounts`].
//! - `InvalidProofs` - The combination of account id, amount, and proofs vector in a claim was invalid.
//! - `MustBeAdmin` - Protected operation, must be performed by admin.
//! - `UnderMinPayout` - The payout amount attempting to be claimed is less than the minimum allowed by [`MinimalPayoutAmount`].
//!
//! ### Dispatchable Functions
//!
//! Callable functions (or extrinsics), also considered as transactions, materialize the
//! pallet contract. Here's the callable functions implemented in this module:
//!
//! - `claim` - Claims tokens awarded through tinlake investments.
//! - `set_upload_account` - Admin function that sets the allowed upload account to add root hashes.
//! - `store_root_hash` - Stores root hash for correspondent claim merkle tree run.
//!
//! ### Public Functions
//! - `sorted_hash_of` - Build a sorted hash of two given hash values.
//!
//! ## Genesis Configuration
//! The pallet is parameterized and configured via [`parameter_types`](https://docs.rs/frame-support/2.0.0-rc1/frame_support/macro.parameter_types.html) macro, at the time the runtime is built
//! by means of the [`construct_runtime`](https://substrate.dev/rustdocs/v3.0.0/frame_support/macro.construct_runtime.html) macro.
//!
//! ## Dependencies
//! This pallet is tightly coupled to:
//! - Substrate FRAME's [balances pallet](https://github.com/paritytech/substrate/tree/master/frame/balances).
//!
//! ## References
//! - [Substrate FRAME v2 attribute macros](https://crates.parity.io/frame_support/attr.pallet.html).
//!
//! ## Credits
//! The Centrifugians Tribe <tribe@centrifuge.io>

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

// Mock runtime and unit test cases
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Extrinsics weight information
mod weights;

// Runtime, system and frame primitives
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{Currency, EnsureOrigin, ExistenceRequirement::KeepAlive, Get},
	weights::Weight,
	PalletId,
};

use frame_system::ensure_root;

use sp_core::Encode;

use sp_runtime::{
	sp_std::{convert::TryInto, vec::Vec},
	traits::{AccountIdConversion, CheckedSub, Hash, SaturatedConversion},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
		ValidTransaction,
	},
};

// Re-export weight information in crate namespace
pub use crate::traits::WeightInfo as PalletWeightInfo;

// Re-export in crate namespace (for runtime construction)
pub use pallet::*;

// ----------------------------------------------------------------------------
// Traits and types declaration
// ----------------------------------------------------------------------------

pub mod traits {

	use super::*;

	/// Weight information for pallet extrinsics
	///
	/// Weights are calculated using runtime benchmarking features.
	/// See [`benchmarking`] module for more information.
	pub trait WeightInfo {
		fn claim(hashes_length: usize) -> Weight;
		fn set_upload_account() -> Weight;
		fn store_root_hash() -> Weight;
	}
} // end of 'traits' module

// ----------------------------------------------------------------------------
// Pallet module
// ----------------------------------------------------------------------------

// Rad claim pallet module
//
// The name of the pallet is provided by `construct_runtime` and is used as
// the unique identifier for the pallet's storage. It is not defined in the
// pallet itself.
#[frame_support::pallet]
pub mod pallet {

	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	// Rad claim pallet type declaration.
	//
	// This structure is a placeholder for traits and functions implementation
	// for the pallet.
	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	// ------------------------------------------------------------------------
	// Pallet configuration
	// ------------------------------------------------------------------------

	/// Claims pallet's configuration trait.
	///
	/// Associated types and constants are declared in this trait. If the pallet
	/// depends on other super-traits, the latter must be added to this trait,
	/// such as, in this case, [`frame_system::Config`] and [`pallet_balances::Config`]
	/// super-traits. Note that [`frame_system::Config`] must always be included.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_balances::Config {
		/// Ensure that origin of a transaction is an administrator.
		type AdminOrigin: EnsureOrigin<Self::Origin>;

		/// Expected currency of the reward claim.
		type Currency: Currency<Self::AccountId>;

		/// Associated type for Event enum
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Expected longevity of a validated unsigned extrinsic.
		///
		/// This parameter is used to determine the longevity of `claim` transaction
		type Longevity: Get<Self::BlockNumber>;

		/// Minimal amount that can be claimed for a reward payout.
		///
		/// This constant is set via [`parameter_types`](https://substrate.dev/docs/en/knowledgebase/runtime/macros#parameter_types)
		/// macro when configuring a runtime.
		#[pallet::constant]
		type MinimalPayoutAmount: Get<node_primitives::Balance>;

		/// Constant configuration parameter to store the module identifier for the pallet.
		///
		/// The module identifier may be of the form ```PalletId(*b"rd/claim")``` and set
		/// using the [`parameter_types`](https://substrate.dev/docs/en/knowledgebase/runtime/macros#parameter_types)
		// macro in the [`runtime/lib.rs`] file.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// A configuration for base priority of unsigned transactions.
		///
		/// This is exposed so that it can be tuned for particular runtime, when
		/// multiple pallets send unsigned transactions.
		type UnsignedPriority: Get<TransactionPriority>;

		/// Weight information for extrinsics in this pallet
		type WeightInfo: PalletWeightInfo;
	}

	// ------------------------------------------------------------------------
	// Pallet events
	// ------------------------------------------------------------------------

	// The macro generates event metadata and derive Clone, Debug, Eq, PartialEq and Codec
	#[pallet::event]
	// The macro generates a function on Pallet to deposit an event
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event triggered after a reward claim is successfully processed
		Claimed(T::AccountId, <T as pallet_balances::Config>::Balance),

		/// Event triggered when the root hash is stored
		RootHashStored(<T as frame_system::Config>::Hash),
	}

	// ------------------------------------------------------------------------
	// Pallet storage items
	// ------------------------------------------------------------------------

	/// Total claimed amounts for all accounts.
	#[pallet::storage]
	#[pallet::getter(fn get_claimed_amount)]
	pub(super) type ClaimedAmounts<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

	/// Map of root hashes that correspond to lists of reward claim amounts per account.
	#[pallet::storage]
	#[pallet::getter(fn get_root_hash)]
	pub(super) type RootHashes<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, bool, ValueQuery>;

	/// Account that is allowed to upload new root hashes.
	#[pallet::storage]
	#[pallet::getter(fn get_upload_account)]
	pub(super) type UploadAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	// ------------------------------------------------------------------------
	// Pallet genesis configuration
	// ------------------------------------------------------------------------

	// The genesis configuration type.
	#[pallet::genesis_config]
	pub struct GenesisConfig {
		// nothing to do folks!!!!
	}

	// The default value for the genesis config type.
	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self {
                // nothing to do folks!!!!
            }
		}
	}

	// The build of genesis for the pallet.
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			// nothing to do folks!!!!
		}
	}

	// ----------------------------------------------------------------------------
	// Pallet lifecycle hooks
	// ----------------------------------------------------------------------------

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	// ------------------------------------------------------------------------
	// Pallet errors
	// ------------------------------------------------------------------------

	#[pallet::error]
	pub enum Error<T> {
		/// Amount being claimed is less than the available amount in [`ClaimedAmounts`].
		InsufficientBalance,

		/// The combination of account id, amount, and proofs vector in a claim was invalid.
		InvalidProofs,

		/// Protected operation, must be performed by admin
		MustBeAdmin,

		/// The payout amount attempting to be claimed is less than the minimum allowed by [`MinimalPayoutAmount`].
		UnderMinPayout,
	}

	// ------------------------------------------------------------------------
	// Pallet dispatchable functions
	// ------------------------------------------------------------------------

	// Declare Call struct and implement dispatchable (or callable) functions.
	//
	// Dispatchable functions are transactions modifying the state of the chain. They
	// are also called extrinsics are constitute the pallet's public interface.
	// Note that each parameter used in functions must implement `Clone`, `Debug`,
	// `Eq`, `PartialEq` and `Codec` traits.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claims tokens awarded through Tinlake investments.
		///
		/// The extrinsic is validated by the custom \[`validate_unsigned`\] function below.
		/// An unsigned transaction is free of fees. We need such an unsigned transaction
		/// as some contributors may not have enough parachain tokens for claiming their
		/// reward payout. The [`validate_unsigned`] function first checks the validity of
		/// this transaction, so that to prevent potential frauds or attacks.
		///
		/// # <weight>
		/// - Based on hashes length
		/// # </weight>
		#[pallet::weight(<T as Config>::WeightInfo::claim(sorted_hashes.len()))]
		pub fn claim(
			origin: OriginFor<T>,
			account_id: T::AccountId,
			amount: T::Balance,
			sorted_hashes: Vec<T::Hash>,
		) -> DispatchResultWithPostInfo {
			// Ensures that this function can only be called via an unsigned transaction
			ensure_none(origin)?;

			ensure!(
				Self::verify_proofs(&account_id, &amount, &sorted_hashes),
				Error::<T>::InvalidProofs
			);

			let claimed = Self::get_claimed_amount(&account_id);

			// Payout = amount - claim
			let payout = amount
				.checked_sub(&claimed)
				.ok_or(Error::<T>::InsufficientBalance)?;

			// Payout must not be less than the minimum allowed
			ensure!(
				payout >= T::MinimalPayoutAmount::get().saturated_into(),
				Error::<T>::UnderMinPayout
			);

			let source = Self::account_id();

			// Transfer payout amount
			<pallet_balances::Pallet<T> as Currency<_>>::transfer(
				&source,
				&account_id,
				payout,
				KeepAlive,
			)?;

			// Set account balance to amount
			ClaimedAmounts::<T>::insert(account_id.clone(), amount);

			Self::deposit_event(Event::Claimed(account_id, amount));

			Ok(().into())
		}

		/// Admin function that sets the allowed upload account to add root hashes
		/// Controlled by custom origin or root
		///
		/// # <weight>
		/// - Based on origin check and write op
		/// # </weight>
		#[pallet::weight(<T as Config>::WeightInfo::set_upload_account())]
		pub fn set_upload_account(
			origin: OriginFor<T>,
			account_id: T::AccountId,
		) -> DispatchResultWithPostInfo {
			Self::can_update_upload_account(origin)?;

			<UploadAccount<T>>::put(account_id);

			Ok(().into())
		}

		/// Stores root hash for correspondent claim Merkle tree run
		///
		/// # <weight>
		/// - Based on origin check and write op
		/// # </weight>
		#[pallet::weight(<T as Config>::WeightInfo::store_root_hash())]
		pub fn store_root_hash(
			origin: OriginFor<T>,
			root_hash: T::Hash,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			ensure!(
				Self::get_upload_account() == Some(who),
				Error::<T>::MustBeAdmin
			);

			<RootHashes<T>>::insert(root_hash, true);

			Self::deposit_event(Event::RootHashStored(root_hash));

			Ok(().into())
		}
	}

	// ------------------------------------------------------------------------
	// Pallet unsigned transactions validation
	// ------------------------------------------------------------------------

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		/// Validate unsigned transactions
		///
		/// Unsigned transactions are generally disallowed. However, since a contributor
		/// claiming a reward payout may not have the necessary tokens on the parachain to
		/// pay the fees of the claim, this `claim` transactions must be unsigned.
		/// Here, we make sure such unsigned, and remember, feeless unsigned transactions
		/// can be used for malicious spams or Deny of Service (DoS) attacks.
		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			if let Call::claim {
				account_id,
				amount,
				sorted_hashes,
			} = call
			{
				// Check that proofs are valid with a root that exists in the root hash storage
				if Self::verify_proofs(account_id, amount, sorted_hashes.into()) {
					return ValidTransaction::with_tag_prefix("RadClaims")
						.priority(T::UnsignedPriority::get())
						.and_provides((account_id, amount, sorted_hashes))
						.longevity(TryInto::<u64>::try_into(T::Longevity::get()).unwrap_or(64_u64))
						.propagate(true)
						.build();
				}
				return InvalidTransaction::BadProof.into();
			}

			InvalidTransaction::Call.into()
		}
	}
} // end of 'pallet' module

// ----------------------------------------------------------------------------
// Pallet implementation block
// ----------------------------------------------------------------------------

// Claims pallet implementation block.
//
// This main implementation block contains two categories of functions, namely:
// - Public functions: These are functions that are `pub` and generally fall into
//   inspector functions that do not write to storage and operation functions that do.
// - Private functions: These are private helpers or utilities that cannot be called
//   from other pallets.
impl<T: Config> Pallet<T> {
	/// Return the account identifier of the claims pallet.
	///
	/// This actually does computation. If you need to keep using it, then make
	/// sure you cache the value and only call this once.
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account()
	}

	/// Build a sorted hash of two given hash values.
	///
	/// Hash a:b if a < b, else b:a. Uses the runtime module's hasher.
	pub fn sorted_hash_of(a: &T::Hash, b: &T::Hash) -> T::Hash {
		let mut h: Vec<u8> = Vec::with_capacity(64);
		if a < b {
			h.extend_from_slice(a.as_ref());
			h.extend_from_slice(b.as_ref());
		} else {
			h.extend_from_slice(b.as_ref());
			h.extend_from_slice(a.as_ref());
		}

		T::Hashing::hash(&h).into()
	}

	/// Returns true if the given origin can update the upload account
	fn can_update_upload_account(origin: T::Origin) -> DispatchResult {
		T::AdminOrigin::try_origin(origin)
			.map(|_| ())
			.or_else(ensure_root)?;

		Ok(())
	}

	// Verifies lexicographically-sorted proofs.
	//
	// This function essentially proceeds as follows, in order to verify proofs:
	// 1. A leaf hash is first built, namely `Hash(account_id + amount)`, with the account and the amount
	// 2. The leaf is then passed to iterator as the first accumulative value to the 'sorted_hash_of' function
	// 3. Then 'sorted_hash_of' function hashes both 'hash1' and 'hash2' together, and the order depends on
	//    which one is "bigger".
	//    This approach avoids having an extra byte that tells if the hash is left or right so they can
	//    be concatenated accordingly before hashing
	// 4. And finally, it checks that the resulting root hash matches with the one stored
	fn verify_proofs(
		account_id: &T::AccountId,
		amount: &T::Balance,
		sorted_hashes: &Vec<T::Hash>,
	) -> bool {
		// Number of proofs should practically never be >30. Checking this
		// blocks abuse.
		if sorted_hashes.len() > 30 {
			return false;
		}

		// Concat account id : amount
		let mut v: Vec<u8> = account_id.encode();
		v.extend(amount.encode());

		// Generate root hash
		let leaf_hash = T::Hashing::hash(&v);
		let mut root_hash = sorted_hashes
			.iter()
			.fold(leaf_hash, |acc, hash| Self::sorted_hash_of(&acc, hash));

		// Initial runs might only have trees of single leaves,
		// in this case leaf_hash is as well root_hash
		if sorted_hashes.len() == 0 {
			root_hash = leaf_hash;
		}

		Self::get_root_hash(root_hash)
	}
}
