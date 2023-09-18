// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use sp_runtime::{DispatchError, DispatchResult};

/// Abstraction that represents a storage where
/// you can subscribe to data updates and collect them
pub trait DataRegistry<DataId, CollectionId> {
	/// A collection of data
	type Collection: DataCollection<DataId, Data = Self::Data>;

	/// Represents a data
	type Data;

	/// Identify the max number a collection can reach.
	#[cfg(feature = "runtime-benchmarks")]
	type MaxCollectionSize: sp_runtime::traits::Get<u32>;

	/// Return the last data value for a data id in a collection
	fn get(data_id: &DataId, collection_id: &CollectionId) -> Result<Self::Data, DispatchError>;

	/// Retrives a collection of data with all data associated to a collection
	/// id
	fn collection(collection_id: &CollectionId) -> Self::Collection;

	/// Start listening data changes for a data id in a collection id
	fn register_id(data_id: &DataId, collection_id: &CollectionId) -> DispatchResult;

	/// Start listening data changes for a data id in a collection id
	fn unregister_id(data_id: &DataId, collection_id: &CollectionId) -> DispatchResult;
}

/// Abstration to represent a collection of data in memory
pub trait DataCollection<DataId> {
	/// Represents a data
	type Data;

	/// Return the last data value for a data id
	fn get(&self, data_id: &DataId) -> Result<Self::Data, DispatchError>;
}
