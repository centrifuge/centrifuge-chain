#![cfg_attr(not(feature = "std"), no_std)]

use node_primitives::{BlockNumber, Hash};
use pallet_anchors::AnchorData;
use sp_api::decl_runtime_apis;

decl_runtime_apis! {
	/// The API to query anchoring info.
	pub trait AnchorApi {
		fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>>;
	}
}
