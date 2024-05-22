use frame_support::{
	traits::{GetStorageVersion, OnRuntimeUpgrade, PalletInfoAccess, StorageVersion},
	weights::{constants::RocksDbWeight, Weight},
};
use sp_runtime::traits::Zero;

const LOG_PREFIX: &str = "LoansExternalWithLinearPricing:";

/// Simply bumps the storage version of a pallet
///
/// NOTE: Use with caution! Must ensure beforehand that a migration is not
/// necessary
pub struct Migration<P, const FROM_VERSION: u16, const TO_VERSION: u16>(
	sp_std::marker::PhantomData<P>,
);
