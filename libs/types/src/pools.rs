use cfg_primitives::Moment;
use codec::{Decode, Encode};
use frame_support::{traits::Get, BoundedVec, RuntimeDebug};
use orml_traits::Change;
use scale_info::TypeInfo;

use super::*;

// #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
// pub struct PoolChanges<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>
// where
// 	MaxTokenNameLength: Get<u32>,
// 	MaxTokenSymbolLength: Get<u32>,
// 	MaxTranches: Get<u32>,
// {
// 	pub tranches: Change<BoundedVec<TrancheUpdate<Rate>, MaxTranches>>,
// 	pub tranche_metadata:
// 		Change<BoundedVec<TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>, MaxTranches>>,
// 	pub min_epoch_time: Change<Moment>,
// 	pub max_nav_age: Change<Moment>,
// }

/// A representation of a pool identifier that can be converted to an account address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolLocator<PoolId> {
	pub pool_id: PoolId,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum PoolChanges {
	Tranches(Change<BoundedVec<TrancheUpdate<Rate>, MaxTranches>>),
	TrancheMetadata,
	MinEpochTime(Change<Moment>),
	MaxNavAge(Change<Moment>),
}