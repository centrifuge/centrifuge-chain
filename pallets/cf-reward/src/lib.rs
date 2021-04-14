//! # Reward module for Centrifuge Parachain Campaigns
//!
//! ## Overview
//!
//! The module does implement the Claim Pallets Reward trait so, that it can be used with alongside.
//! TODO: Describe the rewarding process.
//!
//! ## Callable functions
//!
//! Callable functions, also considered as transactions, materialize the module interface
//! (or contract). Here's the callable functions implemented in this module:
//!

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, decl_storage, decl_event, decl_error, dispatch, weights::{Weight}, Parameter};
use frame_system::{ensure_signed, ensure_root};
use sp_runtime::traits::{Member, MaybeSerialize};
use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize};
use frame_support::dispatch::Codec;
use frame_support::dispatch::fmt::Debug;
use frame_support::traits::{Get, Currency};
use frame_support::dispatch::DispatchResult;
use crowdloan_claim::reward::Reward;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;


pub trait Config: frame_system::Config + pallet_vesting::Config {
    /// Timer after which the Module can be initalized again. This should probably be less than
    /// the maximum lock-time.
    type LeasePeriod: Get<Self::BlockNumber>;
    /// This module emits events, and hence, depends on the runtime's definition of event
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    /// The balance type of the relay chain
    type RelayChainBalance: Parameter + Member + AtLeast32BitUnsigned + Codec + Default + Copy +
        MaybeSerializeDeserialize + Debug;
    /// The currency in which the contributor will be rewarded. Typically this will be the native
    /// currency
    type Currency: Currency<Self::AccountId>;
    /// AccountId of the relay chain
    type RelayChainAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeSerialize + Ord
        + Default;
}

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Callable functions (i.e. transaction) weight trait
pub trait WeightInfo {
    fn initialize() -> Weight;
    fn reward() -> Weight;
}

// Define transaction weights to be used when testing
pub struct TestWeightInfo;
impl WeightInfo for TestWeightInfo {
    fn initialize() -> Weight { 0 }
    fn reward() -> Weight { 0 }
}


decl_event!{
    pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId {
        /// Event emitted when a reward claim was processed successfully.
        RewardClaimed(AccountId),
        /// Event triggered when the reward module is ready to reward contributors
        RewardModuleInitalized,
    }
}


decl_storage! {
    trait Store for Module<T: Config> as Reward {
        LastInit get(fn last_init) : u64;
    }
}


decl_error! {
	pub enum Error for Module<T: Config> {
        /// Not enough funds in the pot for paying a reward payout
        NotEnoughFunds,
    }
}


decl_module! {
    pub struct Module<T: Config> for enum Call where origin: <T as frame_system::Config>::Origin {
        // Activate errors
        type Error = Error<T>;

        // Activate events
        fn deposit_event() = default;

        /// Initialize the module.
        /// Basically this call
        ///
        /// Can/Must be weightless as the call must come from root.
        #[weight = 0]
        fn initialize(origin, amount: BalanceOf<T>) -> DispatchResult {
            ensure_root(origin)?;


            Ok(())
        }
    }
}

impl<T: Config> Module<T> {
    fn placeholder(holder: u32) {
        let x = holder;
    }
}

impl<T: Config> Reward for Module<T> {
    type ParachainAccountId = T::AccountId;
    type ContributionAmount = T::RelayChainBalance;

    fn reward(who: &Self::ParachainAccountId, contribution: &Self::ContributionAmount) -> Result<(), ()> {
        Ok(())
    }
}