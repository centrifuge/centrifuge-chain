#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
    traits::Get,
    weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;
use pallet_proxy_keystore::weights::WeightInfo;

/// Weights for pallet_proxy_keystore using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);

// TODO(cdamian): Update this.
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn create_keystore(_n: u32) -> Weight {
        10 as Weight
    }

    fn add_keys(_n: u32) -> Weight {
        10 as Weight
    }

    fn revoke_keys(_n: u32) -> Weight {
        10 as Weight
    }

    fn set_deposit() -> Weight {
        10 as Weight
    }
}
