use sp_runtime::DispatchResult;

/// Abstraction that represents a storage where
/// you can subscribe to data updates and collect them
pub trait DataRegistry<DataId, CollectionId> {
	/// A collection of data
	type Collection: DataCollection<DataId>;

	/// Represents a data
	type Data;

	/// Return the last data value for a data id
	fn get(data_id: &DataId) -> Self::Data;

	/// Retrives a collection of data with all data associated to a collection id
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
	fn get(&self, data_id: &DataId) -> Self::Data;
}
