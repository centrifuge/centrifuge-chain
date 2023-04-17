use sp_runtime::{DispatchError, DispatchResult};

/// Abstraction that represents a storage where
/// you can subscribe to price updates and collect them
pub trait PriceRegistry {
	/// A price identification
	type PriceId;

	/// A collection identification
	type CollectionId;

	/// A collection of prices
	type Cache: PriceCache<Self::PriceId, Self::Price, Self::Moment>;

	/// Represents a price
	type Price;

	/// Represents a timestamp
	type Moment;

	/// Return the last price value for a price id along with the moment it was updated last time
	fn price(price_id: Self::PriceId)
		-> Result<Option<(Self::Price, Self::Moment)>, DispatchError>;

	/// Retrives a collection of prices with all prices associated to a collection id
	fn cache(collection_id: Self::CollectionId) -> Result<Self::Cache, DispatchError>;

	/// Start listening price changes for a price id in a collection id
	fn register_price_id(
		price_id: Self::PriceId,
		collection_id: Self::CollectionId,
	) -> DispatchResult;

	/// Start listening price changes for a price id in a collection id
	fn unregister_price_id(
		price_id: Self::PriceId,
		collection_id: Self::CollectionId,
	) -> DispatchResult;
}

/// Abstration to represent a cached collection of prices in memory
pub trait PriceCache<PriceId, Price, Moment> {
	/// Return the last price value for a price id along with the moment it was updated last time
	fn price(&self, price_id: PriceId) -> Result<Option<(Price, Moment)>, DispatchError>;
}
