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

//! Traits used and exported by registry pallet.

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

// Runtime, system and frame primitives
use frame_support::{dispatch::DispatchError, weights::Weight};

// ----------------------------------------------------------------------------
// Traits definition
// ----------------------------------------------------------------------------

/// A general interface for registries that require some sort of verification to mint their
/// underlying NFTs. A substrate module can implement this trait.
pub trait VerifierRegistry<AccountId, RegistryId, RegistryInfo, AssetId, AssetInfo, MintInfo> {
	/// Create a new instance of a registry with the associated registry info.
	fn create_new_registry(
		caller: AccountId,
		info: RegistryInfo,
	) -> Result<RegistryId, DispatchError>;

	/// Use the mint info to verify whether the mint is a valid action.
	/// If so, use the asset info to mint an asset.
	fn mint(
		caller: AccountId,
		owner_account: AccountId,
		asset_id: AssetId,
		asset_info: AssetInfo,
		mint_info: MintInfo,
	) -> Result<(), DispatchError>;
}

/// Weight information for pallet extrinsics
///
/// Weights are calculated using runtime benchmarking features.
/// See [`benchmarking`] module for more information.
pub trait WeightInfo {
	fn create_registry() -> Weight;
	fn mint(proofs_length: usize) -> Weight;
}
