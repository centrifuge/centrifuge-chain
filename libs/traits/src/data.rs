use sp_runtime::{DispatchError, DispatchResult};

/// Abstraction that represents a storage where
/// you can subscribe to data updates and collect them
pub trait DataRegistry {
	/// A data identification
	type DataId;

	/// A collection identification
	type CollectionId;

	/// A collection of datas
	type Collection: DataCollection<Self::DataId, Self::Data, Self::Moment>;

	/// Represents a data
	type Data;

	/// Represents a timestamp
	type Moment;

	/// Return the last data value for a data id along with the moment it was updated last time
	fn get(data_id: &Self::DataId) -> Option<(Self::Data, Self::Moment)>;

	/// Retrives a collection of datas with all datas associated to a collection id
	fn collection(collection_id: &Self::CollectionId) -> Self::Collection;

	/// Start listening data changes for a data id in a collection id
	fn register_data_id(
		data_id: &Self::DataId,
		collection_id: &Self::CollectionId,
	) -> DispatchResult;

	/// Start listening data changes for a data id in a collection id
	fn unregister_data_id(
		data_id: &Self::DataId,
		collection_id: &Self::CollectionId,
	) -> DispatchResult;
}

/// Abstration to represent a collection of datas in memory
pub trait DataCollection<DataId, Data, Moment> {
	/// Return the last data value for a data id along with the moment it was updated last time
	fn get(&self, data_id: &DataId) -> Result<Option<(Data, Moment)>, DispatchError>;
}
