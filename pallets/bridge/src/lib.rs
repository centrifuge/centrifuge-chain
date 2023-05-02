// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! # Bridge pallet
//!
//! This pallet implements a bridge between Chainbridge and Centrifuge Chain.

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Pallet extrinsics weight information
pub mod weights;
use cfg_traits::fees::{Fee, Fees};
use chainbridge::types::{ChainId, ResourceId};
// Runtime, system and frame primitives
use frame_support::{
	traits::{Currency, ExistenceRequirement::AllowDeath},
	transactional, PalletId,
};
// Re-export pallet components in crate namespace (for runtime construction)
pub use pallet::*;
use sp_core::U256;
use sp_runtime::traits::{AccountIdConversion, SaturatedConversion};
use sp_std::vec::Vec;
use weights::WeightInfo;

// ----------------------------------------------------------------------------
// Type aliases
// ----------------------------------------------------------------------------

type BalanceOf<T> =
	<<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

// ----------------------------------------------------------------------------
// Pallet module
// ----------------------------------------------------------------------------

// Bridge pallet module
//
// The name of the pallet is provided by `construct_runtime` and is used as
// the unique identifier for the pallet's storage. It is not defined in the
// pallet itself.
#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use super::*;

	pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	// Bridge pallet type declaration.
	//
	// This structure is a placeholder for traits and functions implementation
	// for the pallet.
	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	// ------------------------------------------------------------------------
	// Pallet configuration
	// ------------------------------------------------------------------------

	/// Bridge pallet's configuration trait.
	///
	/// Associated types and constants are declared in this trait. If the pallet
	/// depends on other super-traits, the latter must be added to this trait,
	/// Note that [`frame_system::Config`] must always be included.
	#[pallet::config]
	pub trait Config: frame_system::Config + chainbridge::Config {
		/// Pallet identifier.
		///
		/// The module identifier may be of the form ```PalletId(*b"c/bridge")``` (a string of eight characters)
		/// and set using the [`parameter_types`](https://substrate.dev/docs/en/knowledgebase/runtime/macros#parameter_types)
		/// macro in one of the runtimes (see runtime folder).
		#[pallet::constant]
		type BridgePalletId: Get<PalletId>;

		/// Specifies the origin check provided by the chainbridge for calls
		/// that can only be called by the chainbridge pallet.
		type BridgeOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as frame_system::Config>::AccountId,
		>;

		/// Entity used to pay fees
		type Fees: Fees<
			AccountId = <Self as frame_system::Config>::AccountId,
			Balance = BalanceOf<Self>,
		>;

		/// Currency as viewed from this pallet
		type Currency: Currency<<Self as frame_system::Config>::AccountId>;

		/// Associated type for Event enum
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		// Type for native token ID.
		#[pallet::constant]
		type NativeTokenId: Get<ResourceId>;

		/// Key used to retrieve the fee that are charged when transferring native tokens to target chains.
		#[pallet::constant]
		type NativeTokenTransferFeeKey: Get<<Self::Fees as Fees>::FeeKey>;

		/// Weight information for extrinsics in this pallet
		type WeightInfo: WeightInfo;
	}

	// ------------------------------------------------------------------------
	// Pallet events
	// ------------------------------------------------------------------------

	// The macro generates event metadata and derive Clone, Debug, Eq, PartialEq and Codec
	#[pallet::event]
	// The macro generates a function on Pallet to deposit an event
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Remark(<T as frame_system::Config>::Hash, ResourceId),
	}

	// ------------------------------------------------------------------------
	// Pallet genesis configuration
	// ------------------------------------------------------------------------

	// The genesis configuration type.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub chains: Vec<u8>,
		pub relayers: Vec<<T as frame_system::Config>::AccountId>,
		pub resources: Vec<(ResourceId, Vec<u8>)>,
		pub threshold: u32,
	}

	// The default value for the genesis config type.
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				chains: Default::default(),
				relayers: Default::default(),
				resources: Default::default(),
				threshold: Default::default(),
			}
		}
	}

	// The build of genesis for the pallet.
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			self.chains.iter().for_each(|c| {
				<chainbridge::Pallet<T>>::whitelist(*c).unwrap_or_default();
			});
			self.relayers.iter().for_each(|rs| {
				<chainbridge::Pallet<T>>::register_relayer(rs.clone()).unwrap_or_default();
			});

			self.resources.iter().for_each(|i| {
				let (rid, m) = (i.0, i.1.clone());
				<chainbridge::Pallet<T>>::register_resource(rid, m).unwrap_or_default();
			});

			<chainbridge::Pallet<T>>::set_relayer_threshold(self.threshold).unwrap_or_default();
		}
	}

	// ------------------------------------------------------------------------
	// Pallet errors
	// ------------------------------------------------------------------------

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid transfer
		InvalidTransfer,
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
		/// Transfers some amount of the native token to some recipient on a (whitelisted) destination chain.
		#[pallet::weight(<T as Config>::WeightInfo::transfer_native())]
		#[transactional]
		#[pallet::call_index(0)]
		pub fn transfer_native(
			origin: OriginFor<T>,
			amount: BalanceOf<T>,
			recipient: Vec<u8>,
			dest_id: ChainId,
		) -> DispatchResultWithPostInfo {
			let source = ensure_signed(origin)?;

			ensure!(
				<chainbridge::Pallet<T>>::chain_whitelisted(dest_id),
				Error::<T>::InvalidTransfer
			);

			// Burn additional fees
			T::Fees::fee_to_burn(&source, Fee::Key(T::NativeTokenTransferFeeKey::get()))?;

			let bridge_id = T::BridgePalletId::get().into_account_truncating();
			<T as pallet::Config>::Currency::transfer(&source, &bridge_id, amount, AllowDeath)?;

			let resource_id = T::NativeTokenId::get();
			<chainbridge::Pallet<T>>::transfer_fungible(
				dest_id,
				resource_id,
				recipient,
				// Note: use u128 to restrict balance greater than 128bits
				U256::from(amount.saturated_into::<u128>()),
			)?;

			Ok(().into())
		}

		/// Executes a simple currency transfer using the chainbridge account as the source
		#[pallet::weight(<T as Config>::WeightInfo::transfer())]
		#[transactional]
		#[pallet::call_index(1)]
		pub fn transfer(
			origin: OriginFor<T>,
			to: <T as frame_system::Config>::AccountId,
			amount: BalanceOf<T>,
			_r_id: ResourceId,
		) -> DispatchResultWithPostInfo {
			let source = T::BridgeOrigin::ensure_origin(origin)?;
			<T as pallet::Config>::Currency::transfer(&source, &to, amount, AllowDeath)?;

			Ok(().into())
		}

		/// This can be called by the chainbridge to demonstrate an arbitrary call from a proposal.
		#[pallet::weight(<T as Config>::WeightInfo::remark())]
		#[pallet::call_index(2)]
		pub fn remark(
			origin: OriginFor<T>,
			hash: <T as frame_system::Config>::Hash,
			r_id: ResourceId,
		) -> DispatchResultWithPostInfo {
			T::BridgeOrigin::ensure_origin(origin)?;
			Self::deposit_event(Event::Remark(hash, r_id));

			Ok(().into())
		}
	}
} // end of 'pallet' module
