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

//! Common traits definition module.

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::types::AssetId;

// ----------------------------------------------------------------------------
// Traits declaration
// ----------------------------------------------------------------------------

/// An implementor of this trait *MUST* be an asset of a registry.
/// The registry id that an asset is a member of can be determined
/// when this trait is implemented.
pub trait InRegistry {
	type RegistryId;

	/// Returns the registry id that the self is a member of.
	fn registry_id(&self) -> Self::RegistryId;
}

/// An implementor has an associated asset id that will be used as a
/// unique id within a registry for an asset. Asset ids *MUST* be unique
/// within a registry. Corresponds to a token id in a Centrifuge document.
pub trait HasId {
	/// Returns unique asset id.
	fn id(&self) -> &AssetId;
}
