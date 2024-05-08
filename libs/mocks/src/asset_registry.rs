#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};
	use orml_traits::asset_registry::{AssetMetadata, Inspect, Mutate};
	use sp_std::fmt::Debug;
	use staging_xcm::{v3::prelude::MultiLocation, VersionedMultiLocation};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type AssetId;
		type Balance: Clone + Debug + Eq + PartialEq;
		type CustomMetadata: Parameter + Member + TypeInfo;
		type StringLimit: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_asset_id(f: impl Fn(&MultiLocation) -> Option<T::AssetId> + 'static) {
			register_call!(f);
		}

		pub fn mock_metadata(
			f: impl Fn(
					&T::AssetId,
				) -> Option<AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>>
				+ 'static,
		) {
			register_call!(f);
		}

		pub fn mock_metadata_by_location(
			f: impl Fn(
					&MultiLocation,
				) -> Option<AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>>
				+ 'static,
		) {
			register_call!(f);
		}

		pub fn mock_location(
			f: impl Fn(&T::AssetId) -> Result<Option<MultiLocation>, DispatchError> + 'static,
		) {
			register_call!(f);
		}

		pub fn mock_register_asset(
			f: impl Fn(
					Option<T::AssetId>,
					AssetMetadata<T::Balance, T::CustomMetadata, T::StringLimit>,
				) -> DispatchResult
				+ 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_update_asset(
			f: impl Fn(
					T::AssetId,
					Option<u32>,
					Option<BoundedVec<u8, T::StringLimit>>,
					Option<BoundedVec<u8, T::StringLimit>>,
					Option<T::Balance>,
					Option<Option<VersionedMultiLocation>>,
					Option<T::CustomMetadata>,
				) -> DispatchResult
				+ 'static,
		) {
			register_call!(move |(a, b, c, d, e, g, h)| f(a, b, c, d, e, g, h));
		}
	}

	impl<T: Config> Inspect for Pallet<T> {
		type AssetId = T::AssetId;
		type Balance = T::Balance;
		type CustomMetadata = T::CustomMetadata;
		type StringLimit = T::StringLimit;

		fn asset_id(a: &MultiLocation) -> Option<Self::AssetId> {
			execute_call!(a)
		}

		fn metadata(
			a: &Self::AssetId,
		) -> Option<AssetMetadata<Self::Balance, Self::CustomMetadata, Self::StringLimit>> {
			execute_call!(a)
		}

		fn metadata_by_location(
			a: &MultiLocation,
		) -> Option<AssetMetadata<Self::Balance, Self::CustomMetadata, Self::StringLimit>> {
			execute_call!(a)
		}

		fn location(a: &Self::AssetId) -> Result<Option<MultiLocation>, DispatchError> {
			execute_call!(a)
		}
	}

	impl<T: Config> Mutate for Pallet<T> {
		fn register_asset(
			a: Option<Self::AssetId>,
			b: AssetMetadata<Self::Balance, Self::CustomMetadata, Self::StringLimit>,
		) -> DispatchResult {
			execute_call!((a, b))
		}

		fn update_asset(
			a: Self::AssetId,
			b: Option<u32>,
			c: Option<BoundedVec<u8, Self::StringLimit>>,
			d: Option<BoundedVec<u8, Self::StringLimit>>,
			e: Option<Self::Balance>,
			g: Option<Option<VersionedMultiLocation>>,
			h: Option<Self::CustomMetadata>,
		) -> DispatchResult {
			execute_call!((a, b, c, d, e, g, h))
		}
	}
}
